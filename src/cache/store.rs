use std::path::Path;

use chrono::{DateTime, FixedOffset, Offset, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{Changes, Error, schema};
use crate::{
    inbox::Inbox, message::Message, raw_body::RawBody, sync_state::SyncState, text_part::TextPart,
    transfer_encoding::TransferEncoding,
};

/// The raw bodies kept, in bytes; past it the least recently used go first.
const BODY_BYTES: i64 = 200 * 1024 * 1024;

pub struct Cache {
    connection: Connection,
}

impl Cache {
    pub fn open(dir: &Path) -> Result<Self, Error> {
        Ok(Self {
            connection: schema::open(dir)?,
        })
    }

    /// The newest `limit` rows, all of them when `None`; `None` before the
    /// first resync has written anything.
    pub fn inbox(&self, limit: Option<usize>) -> Result<Option<Inbox>, Error> {
        let unread: Option<usize> = self
            .connection
            .query_row("SELECT unread FROM mailbox", [], |row| row.get(0))
            .optional()?;
        let Some(unread) = unread else {
            return Ok(None);
        };

        // SQLite reads a negative LIMIT as no limit.
        let limit = limit.map_or(-1, |n| i64::try_from(n).unwrap_or(i64::MAX));
        let mut statement = self.connection.prepare_cached(
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
        let mut statement = self
            .connection
            .prepare_cached("SELECT uid FROM message ORDER BY uid")?;
        let uids = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(uids)
    }

    /// Up to `limit` Messages still without a Snippet, newest first, and
    /// the part each one's Snippet is cut from.
    pub fn pending_snippets(&self, limit: usize) -> Result<Vec<(u32, TextPart)>, Error> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare_cached(
            "SELECT uid, text_section, text_encoding, text_charset, text_html FROM message
             WHERE snippet IS NULL ORDER BY received DESC, uid DESC LIMIT ?1",
        )?;
        // A Message with no text part was written with an empty Snippet.
        let pending = statement
            .query_map([limit], |row| Ok((row.get(0)?, text_part(row, 1)?)))?
            .filter_map(|row| row.map(|(uid, part)| part.map(|p| (uid, p))).transpose())
            .collect::<Result<_, _>>()?;
        Ok(pending)
    }

    /// Writes one batch of Snippets in one transaction; a UID no longer
    /// cached is ignored.
    pub fn write_snippets(&self, snippets: &[(u32, String)]) -> Result<(), Error> {
        let transaction = self.connection.unchecked_transaction()?;
        let mut update =
            transaction.prepare_cached("UPDATE message SET snippet = ?2 WHERE uid = ?1")?;
        for (uid, snippet) in snippets {
            update.execute(params![uid, snippet])?;
        }

        drop(update);
        transaction.commit()?;
        Ok(())
    }

    /// The cached body of the Message with this UID, marked as just used;
    /// `None` until it is fetched.
    pub fn body(&self, uid: u32) -> Result<Option<RawBody>, Error> {
        let mut statement = self.connection.prepare_cached(
            "SELECT body.key, body.raw, text_section, text_encoding, text_charset, text_html
             FROM message JOIN body ON body.key = message.body_key WHERE uid = ?1",
        )?;
        let found = statement
            .query_row([uid], |row| {
                let body = RawBody {
                    raw: row.get(1)?,
                    part: text_part(row, 2)?,
                };

                Ok((row.get::<_, i64>(0)?, body))
            })
            .optional()?;
        let Some((key, body)) = found else {
            return Ok(None);
        };

        self.connection
            .prepare_cached(
                "UPDATE body SET used = (SELECT max(used) + 1 FROM body) WHERE key = ?1",
            )?
            .execute([key])?;
        Ok(Some(body))
    }

    /// Stores each `(uid, raw)` under its Message's body key in one
    /// transaction, then evicts past `BODY_BYTES`. A UID no longer cached is
    /// ignored.
    pub fn write_bodies(&self, bodies: &[(u32, Vec<u8>)]) -> Result<(), Error> {
        let transaction = self.connection.unchecked_transaction()?;
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
        transaction.commit()?;
        Ok(())
    }

    pub fn apply(&self, changes: &Changes) -> Result<(), Error> {
        let transaction = self.connection.unchecked_transaction()?;
        if changes.replace {
            transaction.execute("DELETE FROM message", [])?;
        }

        let mut insert = transaction.prepare_cached(
            "INSERT OR REPLACE INTO message
             (uid, body_key, received, received_offset, unread, sender, subject, snippet,
              text_section, text_encoding, text_charset, text_html)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?;
        for new in &changes.new {
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

        let mut delete = transaction.prepare_cached("DELETE FROM message WHERE uid = ?1")?;
        for uid in &changes.expunged {
            delete.execute([uid])?;
        }

        let mut flag =
            transaction.prepare_cached("UPDATE message SET unread = ?2 WHERE uid = ?1")?;
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
        drop((insert, delete, flag));
        transaction.commit()?;
        Ok(())
    }
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
