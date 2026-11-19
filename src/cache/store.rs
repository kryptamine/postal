use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    path::Path,
    sync::Arc,
};

use chrono::{DateTime, FixedOffset, Offset, Utc};
use rusqlite::{
    Connection, OptionalExtension, Row,
    backup::{Backup, StepResult},
    params,
};

use super::{Changes, Error, moves, schema};
use crate::{
    folder::{Folder, Role},
    inbox::Inbox,
    message::Message,
    new_message::NewMessage,
    sync_state::SyncState,
    text_part::TextPart,
    transfer_encoding::TransferEncoding,
};

/// The raw bodies kept, in bytes; past it the least recently used go first.
const BODY_BYTES: i64 = 200 * 1024 * 1024;

/// A Folder's flag changes for the server, each `(uid, unread)`.
pub type FolderFlags = (Arc<str>, Vec<(u32, bool)>);

/// Once a write to disk fails, the Cache moves into memory for the rest of
/// the session, so a full disk or a lost permission stops nothing.
pub struct Cache {
    pub(super) connection: RefCell<Connection>,
    rebuilt: bool,
    in_memory: Cell<bool>,
    unreported_failure: Cell<bool>,
}

impl Cache {
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let (connection, rebuilt) = schema::open(dir)?;
        moves::forget_settled(&connection)?;
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

    /// The newest `limit` rows of `folder`, all of them when `None`; `None`
    /// before its first resync has written it.
    pub fn inbox(&self, folder: &str, limit: Option<usize>) -> Result<Option<Inbox>, Error> {
        let connection = self.connection.borrow();
        let synced = connection
            .prepare_cached("SELECT 1 FROM mailbox WHERE name = ?1 AND uid_validity IS NOT NULL")?
            .query_row([folder], |_| Ok(()))
            .optional()?;
        if synced.is_none() {
            return Ok(None);
        }

        // SQLite reads a negative LIMIT as no limit.
        let limit = limit.map_or(-1, |n| i64::try_from(n).unwrap_or(i64::MAX));
        let mut statement = connection.prepare_cached(
            "SELECT uid, body_key, received, received_offset, unread, sender, subject,
                    coalesce(snippet, ''), thread, from_me
             FROM message WHERE mailbox = ?1
             ORDER BY received DESC, uid DESC LIMIT ?2",
        )?;
        let name: Arc<str> = folder.into();
        let messages = statement
            .query_map(params![folder, limit], |row| {
                Ok(Message {
                    folder: Some(Arc::clone(&name)),
                    ..message(row)?
                })
            })?
            .collect::<Result<_, _>>()?;
        // Only those whose body is cached: that body comes with the Thread alone.
        let mut statement = connection.prepare_cached(
            "SELECT 0, t.key, t.received, t.received_offset, t.unread, t.sender, t.subject,
                    t.snippet, t.thread, t.from_me
             FROM thread_message t JOIN body ON body.key = t.key
             WHERE t.thread IN (SELECT thread FROM message WHERE mailbox = ?1)
               AND t.key NOT IN (SELECT body_key FROM message WHERE mailbox = ?1)
             ORDER BY t.received, t.key",
        )?;
        let elsewhere = statement
            .query_map([folder], message)?
            .collect::<Result<_, _>>()?;
        Ok(Some(Inbox {
            messages,
            elsewhere,
        }))
    }

    /// Every cached Folder's Messages but Trash's and Spam's, newest first,
    /// each once: the copy in `shown` wins, else the first in Folder order.
    /// Their Threads' Messages found elsewhere come as in `inbox`.
    pub fn everywhere(&self, shown: &str) -> Result<Inbox, Error> {
        let mut folders = self.folders()?;
        folders.retain(|f| !matches!(f.role, Some(Role::Trash | Role::Spam)));
        folders.sort_by_key(|f| *f.name != *shown);
        let connection = self.connection.borrow();
        let mut statement = connection.prepare_cached(
            "SELECT uid, body_key, received, received_offset, unread, sender, subject,
                    coalesce(snippet, ''), thread, from_me
             FROM message WHERE mailbox = ?1",
        )?;
        let (mut messages, mut keys) = (Vec::new(), HashSet::new());
        for folder in &folders {
            for row in statement.query_map([&*folder.name], message)? {
                let row = row?;
                if keys.insert(row.body_key) {
                    messages.push(Message {
                        folder: Some(Arc::clone(&folder.name)),
                        ..row
                    });
                }
            }
        }

        messages.sort_unstable_by(|a, b| (b.received, b.uid).cmp(&(a.received, a.uid)));
        let threads: HashSet<u64> = messages.iter().filter_map(|m| m.thread).collect();
        let mut statement = connection.prepare_cached(
            "SELECT 0, t.key, t.received, t.received_offset, t.unread, t.sender, t.subject,
                    t.snippet, t.thread, t.from_me
             FROM thread_message t JOIN body ON body.key = t.key
             ORDER BY t.received, t.key",
        )?;
        let mut elsewhere = Vec::new();
        for row in statement.query_map([], message)? {
            let row = row?;
            if row.thread.is_some_and(|t| threads.contains(&t)) && !keys.contains(&row.body_key) {
                elsewhere.push(row);
            }
        }

        Ok(Inbox {
            messages,
            elsewhere,
        })
    }

    /// Replaces what is cached of Gmail's Thread `id` from other Folders
    /// with `messages`, storing `bodies` by key first, in one transaction.
    pub fn write_thread(
        &self,
        id: u64,
        messages: &[Message],
        bodies: &[(i64, Vec<u8>)],
    ) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            store_bodies(&transaction, bodies)?;
            let id = id.cast_signed();
            transaction.execute("DELETE FROM thread_message WHERE thread = ?1", [id])?;
            let mut insert = transaction.prepare_cached(
                "INSERT OR REPLACE INTO thread_message
                 (key, thread, received, received_offset, unread, sender, subject, from_me,
                  snippet)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for m in messages {
                insert.execute(params![
                    m.body_key,
                    id,
                    m.received.timestamp(),
                    m.received.offset().local_minus_utc(),
                    m.unread,
                    m.sender,
                    m.subject,
                    m.from_me,
                    m.snippet,
                ])?;
            }

            drop(insert);
            evict(&transaction)?;
            transaction.commit()
        })
    }

    pub fn has_body(&self, key: i64) -> Result<bool, Error> {
        let found = self
            .connection
            .borrow()
            .prepare_cached("SELECT 1 FROM body WHERE key = ?1")?
            .query_row([key], |_| Ok(()))
            .optional()?;
        Ok(found.is_some())
    }

    /// Every Folder listed or shown so far, in the Folder Picker's order.
    pub fn folders(&self) -> Result<Vec<Folder>, Error> {
        let connection = self.connection.borrow();
        let mut statement = connection.prepare_cached("SELECT name, role, unread FROM mailbox")?;
        let mut folders = statement
            .query_map([], |row| {
                let role: Option<String> = row.get(1)?;
                let folder = Folder::new(
                    &row.get::<_, String>(0)?,
                    role.and_then(|r| Role::from_label(&r)),
                );
                Ok(Folder {
                    unread: row.get(2)?,
                    ..folder
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        folders.sort_by(Folder::order);
        Ok(folders)
    }

    /// The Folder named `name` as last listed; without a role when it never was.
    pub fn folder(&self, name: &str) -> Result<Folder, Error> {
        let listed = self.folders()?.into_iter().find(|f| *f.name == *name);
        Ok(listed.unwrap_or_else(|| Folder::new(name, None)))
    }

    /// Writes what `LIST` answered: a Folder not in `listed` leaves the
    /// Cache with its rows, and the rest keep what they had.
    pub fn list_folders(&self, listed: &[Folder]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let cached: Vec<String> = transaction
                .prepare_cached("SELECT name FROM mailbox")?
                .query_map([], |row| row.get(0))?
                .collect::<Result<_, _>>()?;
            let mut upsert = transaction.prepare_cached(
                "INSERT INTO mailbox (name, role) VALUES (?1, ?2)
                 ON CONFLICT (name) DO UPDATE SET role = excluded.role",
            )?;
            for folder in listed {
                upsert.execute(params![&*folder.name, folder.role.map(Role::label)])?;
            }

            for gone in cached
                .iter()
                .filter(|c| !listed.iter().any(|f| *f.name == **c))
            {
                forget(&transaction, gone)?;
            }

            drop(upsert);
            transaction.commit()
        })
    }

    /// The Unread count `STATUS` gave for `folder`; one no longer listed is ignored.
    pub fn write_unread(&self, folder: &str, unread: usize) -> Result<(), Error> {
        self.write(|connection| {
            connection
                .prepare_cached("UPDATE mailbox SET unread = ?2 WHERE name = ?1")?
                .execute(params![folder, unread])
                .map(drop)
        })
    }

    /// Drops `folder` and its rows, as for a Folder the server no longer has.
    pub fn remove_folder(&self, folder: &str) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            forget(&transaction, folder)?;
            transaction.commit()
        })
    }

    /// Sets Unread on each `(folder, uid)` and on Gmail's copies of it in
    /// other Folders, adjusting their Unread counts, and queues the change
    /// for the server. Returns whether any row changed.
    pub fn mark(&self, messages: &[(Arc<str>, u32)], unread: bool) -> Result<bool, Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut changed = false;
            for (folder, uid) in messages {
                changed |= mark(&transaction, (folder, *uid), unread)?;
            }

            transaction.commit()?;
            Ok(changed)
        })
    }

    /// The flag changes the server has not acknowledged, by Folder, each
    /// as `(uid, unread)`.
    pub fn pending_flags(&self) -> Result<Vec<FolderFlags>, Error> {
        let connection = self.connection.borrow();
        let mut statement = connection
            .prepare_cached("SELECT mailbox, uid, unread FROM flag_change ORDER BY mailbox, uid")?;
        let mut pending: Vec<FolderFlags> = Vec::new();
        for row in statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get(1)?, row.get(2)?))
        })? {
            let (folder, uid, unread) = row?;
            match pending.last_mut() {
                Some((last, changes)) if **last == *folder => changes.push((uid, unread)),
                _ => pending.push((folder.into(), vec![(uid, unread)])),
            }
        }

        Ok(pending)
    }

    /// Drops `changes` of `folder` from the queue; one changed again since
    /// stays queued.
    pub fn flags_sent(&self, folder: &str, changes: &[(u32, bool)]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut delete = transaction.prepare_cached(
                "DELETE FROM flag_change WHERE mailbox = ?1 AND uid = ?2 AND unread = ?3",
            )?;
            for (uid, unread) in changes {
                delete.execute(params![folder, uid, unread])?;
            }

            drop(delete);
            transaction.commit()
        })
    }

    /// `None` before the first resync of `folder`.
    pub fn state(&self, folder: &str) -> Result<Option<SyncState>, Error> {
        let state = self
            .connection
            .borrow()
            .prepare_cached(
                "SELECT uid_validity, uid_next, highest_modseq FROM mailbox
                 WHERE name = ?1 AND uid_validity IS NOT NULL",
            )?
            .query_row([folder], |row| {
                Ok(SyncState {
                    uid_validity: row.get(0)?,
                    uid_next: row.get(1)?,
                    highest_modseq: row.get(2)?,
                })
            })
            .optional()?;
        Ok(state)
    }

    /// Every UID of `folder` the server holds as the Cache knows it,
    /// ascending: a queued Delete's too, an Undo's old one not.
    pub fn uids(&self, folder: &str) -> Result<Vec<u32>, Error> {
        let connection = self.connection.borrow();
        let mut statement = connection.prepare_cached(
            "SELECT uid FROM message WHERE mailbox = ?1 AND NOT EXISTS (SELECT 1 FROM undone u
                 WHERE u.mailbox = ?1 AND u.uid = message.uid AND (u.at_trash OR u.at_uid != u.uid))
             UNION SELECT uid FROM deleted WHERE mailbox = ?1
             ORDER BY uid",
        )?;
        let uids = statement
            .query_map([folder], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(uids)
    }

    pub fn oldest_uid(&self, folder: &str) -> Result<Option<u32>, Error> {
        let oldest = self
            .connection
            .borrow()
            .prepare_cached("SELECT min(uid) FROM message WHERE mailbox = ?1")?
            .query_row([folder], |row| row.get(0))?;
        Ok(oldest)
    }

    /// Up to `limit` Messages of `folder` still without a Snippet, newest
    /// first, and the part each one's Snippet is cut from; only UIDs below
    /// `below` when given.
    pub fn pending_snippets(
        &self,
        folder: &str,
        limit: usize,
        below: Option<u32>,
    ) -> Result<Vec<(u32, TextPart)>, Error> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let connection = self.connection.borrow();
        let mut statement = connection.prepare_cached(
            "SELECT uid, text_section, text_encoding, text_charset, text_html FROM message
             WHERE mailbox = ?3 AND snippet IS NULL AND (?2 IS NULL OR uid < ?2)
             ORDER BY received DESC, uid DESC LIMIT ?1",
        )?;
        // A Message with no text part was written with an empty Snippet.
        let pending = statement
            .query_map(params![limit, below, folder], |row| {
                Ok((row.get(0)?, text_part(row, 1)?))
            })?
            .filter_map(|row| row.map(|(uid, part)| part.map(|p| (uid, p))).transpose())
            .collect::<Result<_, _>>()?;
        Ok(pending)
    }

    /// Writes one batch of Snippets in one transaction; a UID no longer
    /// cached is ignored.
    pub fn write_snippets(&self, folder: &str, snippets: &[(u32, String)]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut update = transaction.prepare_cached(
                "UPDATE message SET snippet = ?2 WHERE mailbox = ?3 AND uid = ?1",
            )?;
            for (uid, snippet) in snippets {
                update.execute(params![uid, snippet, folder])?;
            }

            drop(update);
            transaction.commit()
        })
    }

    /// The body cached under `key`, marked as just used; `None` until it is
    /// fetched.
    pub fn body(&self, key: i64) -> Result<Option<Vec<u8>>, Error> {
        let found = self
            .connection
            .borrow()
            .prepare_cached("SELECT raw FROM body WHERE key = ?1")?
            .query_row([key], |row| row.get(0))
            .optional()?;
        let Some(body) = found else {
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
    pub fn write_bodies(&self, folder: &str, bodies: &[(u32, Vec<u8>)]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut insert = transaction.prepare_cached(
                "INSERT OR REPLACE INTO body (key, raw, size, used)
                 SELECT body_key, ?2, ?3, (SELECT coalesce(max(used), 0) + 1 FROM body)
                 FROM message WHERE mailbox = ?4 AND uid = ?1",
            )?;
            for (uid, raw) in bodies {
                insert.execute(params![uid, raw, raw.len(), folder])?;
            }

            drop(insert);
            evict(&transaction)?;
            transaction.commit()
        })
    }

    pub fn apply(&self, folder: &str, changes: &Changes) -> Result<(), Error> {
        self.write(|connection| apply(connection, folder, changes))
    }

    /// Adds rows older than every cached one in one transaction, leaving
    /// the Folder's state as the last resync wrote it.
    pub fn backfill(&self, folder: &str, older: &[NewMessage]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            insert(&transaction, folder, older)?;
            transaction.commit()
        })
    }

    /// Runs `write` on disk; when that fails, copies what the disk holds
    /// into memory and runs it there. `write` must leave nothing behind
    /// when it fails, as a transaction does.
    pub(super) fn write<T>(
        &self,
        write: impl Fn(&Connection) -> rusqlite::Result<T>,
    ) -> Result<T, Error> {
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

fn apply(connection: &Connection, folder: &str, changes: &Changes) -> rusqlite::Result<()> {
    let transaction = connection.unchecked_transaction()?;
    if changes.replace {
        transaction.execute("DELETE FROM message WHERE mailbox = ?1", [folder])?;
        transaction.execute("DELETE FROM flag_change WHERE mailbox = ?1", [folder])?;
        transaction.execute("DELETE FROM moved WHERE mailbox = ?1", [folder])?;
    }

    insert(&transaction, folder, &changes.new)?;
    moves::settle(&transaction, folder)?;
    // A row an Undo put back stays: the expunge is its move's.
    let mut delete = transaction.prepare_cached(
        "DELETE FROM message WHERE mailbox = ?1 AND uid = ?2
           AND NOT EXISTS (SELECT 1 FROM undone WHERE mailbox = ?1 AND uid = ?2)",
    )?;
    for uid in &changes.expunged {
        delete.execute(params![folder, uid])?;
    }

    // The server's flag is older than a change still queued for it.
    let mut flag = transaction.prepare_cached(
        "UPDATE message SET unread = ?3 WHERE mailbox = ?1 AND uid = ?2
           AND NOT EXISTS (SELECT 1 FROM flag_change WHERE mailbox = ?1 AND uid = ?2)",
    )?;
    for (uid, unread) in &changes.flags {
        flag.execute(params![folder, uid, unread])?;
    }

    let state = &changes.state;
    transaction.execute(
        "INSERT INTO mailbox (name, uid_validity, uid_next, highest_modseq, unread)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (name) DO UPDATE SET uid_validity = excluded.uid_validity,
             uid_next = excluded.uid_next, highest_modseq = excluded.highest_modseq,
             unread = excluded.unread",
        params![
            folder,
            state.uid_validity,
            state.uid_next,
            state.highest_modseq,
            changes.unread,
        ],
    )?;
    drop((delete, flag));
    transaction.commit()
}

/// Drops `folder` with its rows and its queued changes.
fn forget(connection: &Connection, folder: &str) -> rusqlite::Result<()> {
    connection.execute("DELETE FROM message WHERE mailbox = ?1", [folder])?;
    connection.execute("DELETE FROM flag_change WHERE mailbox = ?1", [folder])?;
    connection.execute("DELETE FROM moved WHERE mailbox = ?1", [folder])?;
    connection.execute("DELETE FROM mailbox WHERE name = ?1", [folder])?;
    Ok(())
}

/// One Message of `Cache::mark`; false when it is not cached or already so.
fn mark(
    connection: &Connection,
    (folder, uid): (&str, u32),
    unread: bool,
) -> rusqlite::Result<bool> {
    let key: Option<i64> = connection
        .prepare_cached(
            "SELECT body_key FROM message WHERE mailbox = ?1 AND uid = ?2 AND unread != ?3",
        )?
        .query_row(params![folder, uid, unread], |row| row.get(0))
        .optional()?;
    let Some(key) = key else {
        return Ok(false);
    };

    // Only Gmail's body key names one Message in every Folder, and its
    // `\Seen` is shared by them all.
    let copies = "(mailbox = ?1 AND uid = ?2) OR (body_key = ?3 AND thread IS NOT NULL)";
    connection
        .prepare_cached(&format!(
            "UPDATE mailbox SET unread = max(unread + ?4, 0)
             WHERE name IN (SELECT mailbox FROM message WHERE ({copies}) AND unread != ?5)"
        ))?
        .execute(params![
            folder,
            uid,
            key,
            if unread { 1 } else { -1 },
            unread
        ])?;
    connection
        .prepare_cached(&format!(
            "UPDATE message SET unread = ?4 WHERE ({copies}) AND unread != ?4"
        ))?
        .execute(params![folder, uid, key, unread])?;
    connection
        .prepare_cached("UPDATE thread_message SET unread = ?2 WHERE key = ?1")?
        .execute(params![key, unread])?;
    connection
        .prepare_cached(
            "INSERT OR REPLACE INTO flag_change (mailbox, uid, unread) VALUES (?1, coalesce(
                 (SELECT at_uid FROM undone WHERE mailbox = ?1 AND uid = ?2 AND NOT at_trash), ?2
             ), ?3)",
        )?
        .execute(params![folder, uid, unread])?;
    Ok(true)
}

fn store_bodies(connection: &Connection, bodies: &[(i64, Vec<u8>)]) -> rusqlite::Result<()> {
    let mut insert = connection.prepare_cached(
        "INSERT OR REPLACE INTO body (key, raw, size, used)
         VALUES (?1, ?2, ?3, (SELECT coalesce(max(used), 0) + 1 FROM body))",
    )?;
    for (key, raw) in bodies {
        insert.execute(params![key, raw, raw.len()])?;
    }

    Ok(())
}

/// Drops the least recently used bodies past `BODY_BYTES`.
fn evict(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute(
        "DELETE FROM body WHERE key IN (
             SELECT key FROM (
                 SELECT key, sum(size) OVER (ORDER BY used DESC) AS kept FROM body
             ) WHERE kept > ?1
         )",
        [BODY_BYTES],
    )?;
    Ok(())
}

/// A row whose Delete the server has yet to see stays out: the server
/// still lists it in `folder`.
fn insert(connection: &Connection, folder: &str, rows: &[NewMessage]) -> rusqlite::Result<()> {
    let mut insert = connection.prepare_cached(
        "INSERT OR REPLACE INTO message
         (uid, body_key, received, received_offset, unread, sender, subject, snippet,
          text_section, text_encoding, text_charset, text_html, mailbox, thread, from_me)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
         WHERE NOT EXISTS (SELECT 1 FROM deleted WHERE mailbox = ?13 AND uid = ?1)",
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
            folder,
            m.thread.map(u64::cast_signed),
            m.from_me,
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
        thread: row.get::<_, Option<i64>>(8)?.map(i64::cast_unsigned),
        from_me: row.get(9)?,
        folder: None,
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
