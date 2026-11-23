use std::{collections::HashSet, path::Path};

use chrono::{DateTime, FixedOffset, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use super::{Error, ThreadOf, received::received, schema};
use crate::{
    domain::answering::Answering, domain::body_key::BodyKey, domain::folder::Role,
    domain::message_ref::MessageRef,
};

/// The Cache for reading alone, beside a TUI that may be writing it: it
/// can neither rebuild nor write, and reads every Account's rows at once.
pub struct Reader {
    pub(super) connection: Connection,
}

/// An address as one Message named it.
pub struct Seen {
    /// The Account's address, lowercased.
    pub account: String,
    pub address: String,
    pub name: Option<String>,
    pub written: bool,
    pub at: DateTime<Utc>,
}

/// The columns `thread_message` reads after the body key, named alike in
/// the `message` and `thread_message` tables.
const THREAD_ROW: &str = "received, received_offset, from_line, to_line, subject";

/// One Message of a Thread as the Cache holds it.
pub struct ThreadMessage {
    pub body_key: BodyKey,
    /// As `Message::from` and `Message::to` hold them.
    pub from: String,
    pub to: String,
    pub subject: String,
    pub received: DateTime<FixedOffset>,
    /// `None` when its body is not cached.
    pub raw: Option<Vec<u8>>,
}

impl Reader {
    /// `NotReady` unless the Cache in `dir` has this schema.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        Ok(Self {
            connection: schema::open_read_only(dir)?,
        })
    }

    /// What every Account's mail says of its Correspondents.
    pub fn correspondence(&self) -> Result<Vec<Seen>, Error> {
        let mut statement = self
            .connection
            .prepare("SELECT account, address, name, written, at FROM correspondence")?;
        let seen = statement
            .query_map([], |row| {
                Ok(Seen {
                    account: row.get(0)?,
                    address: row.get(1)?,
                    name: row.get(2)?,
                    written: row.get(3)?,
                    at: DateTime::from_timestamp(row.get(4)?, 0).unwrap_or_default(),
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(seen)
    }

    /// The Messages of the Thread `answering` names in the Account
    /// `account`, as `thread_of` reads them, or the one Message it answers
    /// without one; `None` when it names neither.
    pub fn thread(
        &self,
        account: &str,
        answering: &Answering,
    ) -> Result<Option<Vec<ThreadMessage>>, Error> {
        match (answering.thread, &answering.original) {
            (Some(thread), _) => self.thread_of(account, ThreadOf::Gmail(thread)).map(Some),
            (None, Some(MessageRef { folder, uid })) => {
                let mut statement = self.connection.prepare(&format!(
                    "SELECT body_key, {THREAD_ROW}
                     FROM message WHERE account = ?1 AND mailbox = ?2 AND uid = ?3"
                ))?;
                let rows = statement
                    .query_map(params![account, folder, uid], thread_message)?
                    .collect::<Result<_, _>>()?;
                self.with_bodies(account, rows).map(Some)
            }
            (None, None) => Ok(None),
        }
    }

    /// The Messages of the Thread `of` in the Account `account`, oldest
    /// first, each once, none from Trash or Spam unless it is all there.
    pub fn thread_of(&self, account: &str, of: ThreadOf) -> Result<Vec<ThreadMessage>, Error> {
        let (trash, spam) = (Role::Trash.label(), Role::Spam.label());
        let (sql, key) = match of {
            ThreadOf::Gmail(thread) => (
                format!(
                    "SELECT body_key, {THREAD_ROW}
                     FROM message WHERE account = ?1 AND thread = ?2 AND {KEPT}
                     UNION ALL
                     SELECT key, {THREAD_ROW}
                     FROM thread_message WHERE account = ?1 AND thread = ?2
                     ORDER BY 2, 1"
                ),
                thread.get().cast_signed(),
            ),
            ThreadOf::Root(root) => (
                format!(
                    "SELECT body_key, {THREAD_ROW}
                     FROM message WHERE account = ?1 AND root = ?2 AND {KEPT} ORDER BY 2, 1"
                ),
                root.get().cast_signed(),
            ),
            ThreadOf::Lone(body) => (
                format!(
                    "SELECT body_key, {THREAD_ROW}
                     FROM message WHERE account = ?1 AND body_key = ?2 AND {KEPT}"
                ),
                body.get(),
            ),
        };

        let mut statement = self.connection.prepare(&sql)?;
        let mut rows: Vec<ThreadMessage> = statement
            .query_map(params![account, key, trash, spam, false], thread_message)?
            .collect::<Result<_, _>>()?;
        if rows.is_empty() {
            rows = statement
                .query_map(params![account, key, trash, spam, true], thread_message)?
                .collect::<Result<_, _>>()?;
        }

        self.with_bodies(account, rows)
    }

    /// `rows` each once, with its body when cached.
    fn with_bodies(
        &self,
        account: &str,
        rows: Vec<ThreadMessage>,
    ) -> Result<Vec<ThreadMessage>, Error> {
        let mut keys = HashSet::new();
        let mut kept = Vec::new();
        for mut message in rows {
            if keys.insert(message.body_key) {
                message.raw = self.body(account, message.body_key)?;
                kept.push(message);
            }
        }

        Ok(kept)
    }

    /// The raw body of `account`'s Message `key`; `None` when not cached.
    pub fn body(&self, account: &str, key: BodyKey) -> Result<Option<Vec<u8>>, Error> {
        Ok(self
            .connection
            .prepare_cached("SELECT raw FROM body WHERE account = ?1 AND key = ?2")?
            .query_row(params![account, key], |row| row.get(0))
            .optional()?)
    }
}

/// A row outside Trash and Spam, `?3` and `?4`, or any row when `?5` is true.
const KEPT: &str = "(?5 OR mailbox NOT IN (SELECT name FROM mailbox
                                           WHERE account = ?1 AND role IN (?3, ?4)))";

/// A row of the body key, then `THREAD_ROW`, as its Message without a body.
fn thread_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<ThreadMessage> {
    Ok(ThreadMessage {
        body_key: row.get(0)?,
        received: received(row, 1)?,
        from: row.get(3)?,
        to: row.get(4)?,
        subject: row.get(5)?,
        raw: None,
    })
}
