use std::path::Path;

use chrono::{DateTime, FixedOffset, Offset, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{Changes, Error, schema};
use crate::{
    inbox::Inbox, message::Message, sync_state::SyncState, text_part::TextPart,
    transfer_encoding::TransferEncoding,
};

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
            "SELECT uid, received, received_offset, unread, sender, subject, coalesce(snippet, '')
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
        let pending = statement
            .query_map([limit], |row| {
                let encoding: String = row.get(2)?;
                let part = TextPart {
                    section: row.get(1)?,
                    encoding: TransferEncoding::from_label(&encoding),
                    charset: row.get(3)?,
                    html: row.get(4)?,
                };

                Ok((row.get(0)?, part))
            })?
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

    pub fn apply(&self, changes: &Changes) -> Result<(), Error> {
        let transaction = self.connection.unchecked_transaction()?;
        if changes.replace {
            transaction.execute("DELETE FROM message", [])?;
        }

        let mut insert = transaction.prepare_cached(
            "INSERT OR REPLACE INTO message
             (uid, received, received_offset, unread, sender, subject, snippet,
              text_section, text_encoding, text_charset, text_html)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?;
        for new in &changes.new {
            let (m, part) = (&new.row, new.text_part.as_ref());
            insert.execute(params![
                m.uid,
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
    let offset = FixedOffset::east_opt(row.get(2)?).unwrap_or_else(|| Utc.fix());
    let received = DateTime::from_timestamp(row.get(1)?, 0)
        .unwrap_or_default()
        .with_timezone(&offset);
    Ok(Message {
        uid: row.get(0)?,
        received,
        unread: row.get(3)?,
        sender: row.get(4)?,
        subject: row.get(5)?,
        snippet: row.get(6)?,
    })
}
