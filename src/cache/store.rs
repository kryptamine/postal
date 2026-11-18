use std::{
    cell::{Cell, RefCell},
    path::Path,
};

use chrono::{DateTime, FixedOffset, Offset, Utc};
use rusqlite::{
    Connection, OptionalExtension, Row,
    backup::{Backup, StepResult},
    params,
};

use super::{Changes, Error, schema};
use crate::{
    inbox::Inbox, message::Message, new_message::NewMessage, sync_state::SyncState,
    text_part::TextPart, transfer_encoding::TransferEncoding,
};

/// The raw bodies kept, in bytes; past it the least recently used go first.
const BODY_BYTES: i64 = 200 * 1024 * 1024;

/// Once a write to disk fails, the Cache moves into memory for the rest of
/// the session, so a full disk or a lost permission stops nothing.
pub struct Cache {
    connection: RefCell<Connection>,
    rebuilt: bool,
    in_memory: Cell<bool>,
    unreported_failure: Cell<bool>,
}

impl Cache {
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let (connection, rebuilt) = schema::open(dir)?;
        Ok(Self {
            connection: RefCell::new(connection),
            rebuilt,
            in_memory: Cell::new(false),
            unreported_failure: Cell::new(false),
        })
    }

    /// Whether opening replaced a corrupt or outdated Cache.
    pub const fn rebuilt(&self) -> bool {
        self.rebuilt
    }

    /// Whether a write to disk failed since the last call. True at most
    /// once a session, as the Cache stays in memory from then on.
    pub const fn take_write_failure(&self) -> bool {
        self.unreported_failure.replace(false)
    }

    /// The newest `limit` rows, all of them when `None`; `None` before the
    /// first resync has written anything.
    pub fn inbox(&self, limit: Option<usize>) -> Result<Option<Inbox>, Error> {
        let connection = self.connection.borrow();
        let unread: Option<usize> = connection
            .query_row("SELECT unread FROM mailbox", [], |row| row.get(0))
            .optional()?;
        let Some(unread) = unread else {
            return Ok(None);
        };

        // SQLite reads a negative LIMIT as no limit.
        let limit = limit.map_or(-1, |n| i64::try_from(n).unwrap_or(i64::MAX));
        let mut statement = connection.prepare_cached(
            "SELECT uid, body_key, received, received_offset, unread, sender, subject,
                    coalesce(snippet, '')
             FROM message
             ORDER BY received DESC, uid DESC LIMIT ?1",
        )?;
        let messages = statement
            .query_map([limit], message)?
            .collect::<Result<_, _>>()?;
        Ok(Some(Inbox { messages, unread }))
    }

    pub fn state(&self) -> Result<Option<SyncState>, Error> {
        let state = self
            .connection
            .borrow()
            .query_row(
                "SELECT uid_validity, uid_next, highest_modseq FROM mailbox",
                [],
                |row| {
                    Ok(SyncState {
                        uid_validity: row.get(0)?,
                        uid_next: row.get(1)?,
                        highest_modseq: row.get(2)?,
                    })
                },
            )
            .optional()?;
        Ok(state)
    }

    /// Every cached UID, ascending.
    pub fn uids(&self) -> Result<Vec<u32>, Error> {
        let connection = self.connection.borrow();
        let mut statement = connection.prepare_cached("SELECT uid FROM message ORDER BY uid")?;
        let uids = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(uids)
    }

    pub fn oldest_uid(&self) -> Result<Option<u32>, Error> {
        let oldest =
            self.connection
                .borrow()
                .query_row("SELECT min(uid) FROM message", [], |row| row.get(0))?;
        Ok(oldest)
    }

    /// Up to `limit` Messages still without a Snippet, newest first, and
    /// the part each one's Snippet is cut from; only UIDs below `below`
    /// when given.
    pub fn pending_snippets(
        &self,
        limit: usize,
        below: Option<u32>,
    ) -> Result<Vec<(u32, TextPart)>, Error> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let connection = self.connection.borrow();
        let mut statement = connection.prepare_cached(
            "SELECT uid, text_section, text_encoding, text_charset, text_html FROM message
             WHERE snippet IS NULL AND (?2 IS NULL OR uid < ?2)
             ORDER BY received DESC, uid DESC LIMIT ?1",
        )?;
        // A Message with no text part was written with an empty Snippet.
        let pending = statement
            .query_map(params![limit, below], |row| {
                Ok((row.get(0)?, text_part(row, 1)?))
            })?
            .filter_map(|row| row.map(|(uid, part)| part.map(|p| (uid, p))).transpose())
            .collect::<Result<_, _>>()?;
        Ok(pending)
    }

    /// Writes one batch of Snippets in one transaction; a UID no longer
    /// cached is ignored.
    pub fn write_snippets(&self, snippets: &[(u32, String)]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut update =
                transaction.prepare_cached("UPDATE message SET snippet = ?2 WHERE uid = ?1")?;
            for (uid, snippet) in snippets {
                update.execute(params![uid, snippet])?;
            }

            drop(update);
            transaction.commit()
        })
    }

    /// The cached body of the Message with this UID, marked as just used;
    /// `None` until it is fetched.
    pub fn body(&self, uid: u32) -> Result<Option<Vec<u8>>, Error> {
        let found = self
            .connection
            .borrow()
            .prepare_cached(
                "SELECT body.key, body.raw
                 FROM message JOIN body ON body.key = message.body_key WHERE uid = ?1",
            )?
            .query_row([uid], |row| Ok((row.get::<_, i64>(0)?, row.get(1)?)))
            .optional()?;
        let Some((key, body)) = found else {
            return Ok(None);
        };

        self.write(|connection| {
            connection
                .prepare_cached(
                    "UPDATE body SET used = (SELECT max(used) + 1 FROM body) WHERE key = ?1",
                )?
                .execute([key])
                .map(drop)
        })?;
        Ok(Some(body))
    }

    /// Stores each `(uid, raw)` under its Message's body key in one
    /// transaction, then evicts past `BODY_BYTES`. A UID no longer cached is
    /// ignored.
    pub fn write_bodies(&self, bodies: &[(u32, Vec<u8>)]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut insert = transaction.prepare_cached(
                "INSERT OR REPLACE INTO body (key, raw, size, used)
                 SELECT body_key, ?2, ?3, (SELECT coalesce(max(used), 0) + 1 FROM body)
                 FROM message WHERE uid = ?1",
            )?;
            for (uid, raw) in bodies {
                insert.execute(params![uid, raw, raw.len()])?;
            }

            transaction.execute(
                "DELETE FROM body WHERE key IN (
                     SELECT key FROM (
                         SELECT key, sum(size) OVER (ORDER BY used DESC) AS kept FROM body
                     ) WHERE kept > ?1
                 )",
                [BODY_BYTES],
            )?;
            drop(insert);
            transaction.commit()
        })
    }

    pub fn apply(&self, changes: &Changes) -> Result<(), Error> {
        self.write(|connection| apply(connection, changes))
    }

    /// Adds rows older than every cached one in one transaction, leaving
    /// the Inbox's state as the last resync wrote it.
    pub fn backfill(&self, older: &[NewMessage]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            insert(&transaction, older)?;
            transaction.commit()
        })
    }

    /// Runs `write` on disk; when that fails, copies what the disk holds
    /// into memory and runs it there. `write` must leave nothing behind
    /// when it fails, as a transaction does.
    fn write<T>(&self, write: impl Fn(&Connection) -> rusqlite::Result<T>) -> Result<T, Error> {
        let error = match write(&self.connection.borrow()) {
            Ok(done) => return Ok(done),
            Err(error) if self.in_memory.get() => return Err(error.into()),
            Err(error) => error,
        };

        tracing::error!("cache write failed, keeping the Cache in memory: {error}");
        let mut memory = Connection::open_in_memory()?;
        let copied = Backup::new(&self.connection.borrow(), &mut memory)?.step(-1)?;
        if copied != StepResult::Done {
            return Err(error.into());
        }

        *self.connection.borrow_mut() = memory;
        self.in_memory.set(true);
        self.unreported_failure.set(true);
        Ok(write(&self.connection.borrow())?)
    }
}

fn apply(connection: &Connection, changes: &Changes) -> rusqlite::Result<()> {
    let transaction = connection.unchecked_transaction()?;
    if changes.replace {
        transaction.execute("DELETE FROM message", [])?;
    }

    insert(&transaction, &changes.new)?;
    let mut delete = transaction.prepare_cached("DELETE FROM message WHERE uid = ?1")?;
    for uid in &changes.expunged {
        delete.execute([uid])?;
    }

    let mut flag = transaction.prepare_cached("UPDATE message SET unread = ?2 WHERE uid = ?1")?;
    for (uid, unread) in &changes.flags {
        flag.execute(params![uid, unread])?;
    }

    let state = &changes.state;
    transaction.execute(
        "INSERT OR REPLACE INTO mailbox (id, uid_validity, uid_next, highest_modseq, unread)
         VALUES (0, ?1, ?2, ?3, ?4)",
        params![
            state.uid_validity,
            state.uid_next,
            state.highest_modseq,
            changes.unread,
        ],
    )?;
    drop((delete, flag));
    transaction.commit()
}

fn insert(connection: &Connection, rows: &[NewMessage]) -> rusqlite::Result<()> {
    let mut insert = connection.prepare_cached(
        "INSERT OR REPLACE INTO message
         (uid, body_key, received, received_offset, unread, sender, subject, snippet,
          text_section, text_encoding, text_charset, text_html)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;
    for new in rows {
        let (m, part) = (&new.row, new.text_part.as_ref());
        insert.execute(params![
            m.uid,
            m.body_key,
            m.received.timestamp(),
            m.received.offset().local_minus_utc(),
            m.unread,
            m.sender,
            m.subject,
            part.is_none().then_some(""),
            part.map(|p| &p.section),
            part.map(|p| p.encoding.label()),
            part.map(|p| &p.charset),
            part.map(|p| p.html),
        ])?;
    }

    Ok(())
}

fn message(row: &Row<'_>) -> rusqlite::Result<Message> {
    let offset = FixedOffset::east_opt(row.get(3)?).unwrap_or_else(|| Utc.fix());
    let received = DateTime::from_timestamp(row.get(2)?, 0)
        .unwrap_or_default()
        .with_timezone(&offset);
    Ok(Message {
        uid: row.get(0)?,
        body_key: row.get(1)?,
        received,
        unread: row.get(4)?,
        sender: row.get(5)?,
        subject: row.get(6)?,
        snippet: row.get(7)?,
    })
}

/// The text part in the four columns from `first`: section, encoding,
/// charset and html; `None` when the section is NULL.
fn text_part(row: &Row<'_>, first: usize) -> rusqlite::Result<Option<TextPart>> {
    let Some(section) = row.get::<_, Option<String>>(first)? else {
        return Ok(None);
    };

    let encoding: String = row.get(first + 1)?;
    Ok(Some(TextPart {
        section,
        encoding: TransferEncoding::from_label(&encoding),
        charset: row.get(first + 2)?,
        html: row.get(first + 3)?,
    }))
}
