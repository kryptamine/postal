use std::{collections::HashSet, path::Path};

use chrono::{DateTime, FixedOffset, Offset, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use super::{Error, schema};
use crate::{answering::Answering, body_key::BodyKey, folder::Role};

/// The Cache for reading alone, beside a TUI that may be writing it: it
/// can neither rebuild nor write, and reads every Account's rows at once.
pub struct Reader {
    connection: Connection,
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
const THREAD_ROW: &str = "received, received_offset, from_line, to_line";

/// One Message of a Thread as the Cache holds it.
pub struct ThreadMessage {
    /// As `Message::from` and `Message::to` hold them.
    pub from: String,
    pub to: String,
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
    /// `account`, oldest first, each once, none from Trash or Spam; `None` when it
    /// names neither a Thread nor a Message.
    pub fn thread(
        &self,
        account: &str,
        answering: &Answering,
    ) -> Result<Option<Vec<ThreadMessage>>, Error> {
        let rows: Vec<(BodyKey, ThreadMessage)> = match (answering.thread, &answering.original) {
            (Some(thread), _) => {
                let mut statement = self.connection.prepare(&format!(
                    "SELECT body_key, {THREAD_ROW}
                     FROM message WHERE account = ?1 AND thread = ?2
                       AND mailbox NOT IN (SELECT name FROM mailbox
                                           WHERE account = ?1 AND role IN (?3, ?4))
                     UNION ALL
                     SELECT key, {THREAD_ROW}
                     FROM thread_message WHERE account = ?1 AND thread = ?2
                     ORDER BY 2, 1"
                ))?;
                let args = params![account, thread, Role::Trash.label(), Role::Spam.label()];
                statement
                    .query_map(args, thread_message)?
                    .collect::<Result<_, _>>()?
            }
            (None, Some((folder, uid))) => {
                let mut statement = self.connection.prepare(&format!(
                    "SELECT body_key, {THREAD_ROW}
                     FROM message WHERE account = ?1 AND mailbox = ?2 AND uid = ?3"
                ))?;
                statement
                    .query_map(params![account, &**folder, uid], thread_message)?
                    .collect::<Result<_, _>>()?
            }
            (None, None) => return Ok(None),
        };

        let mut body = self
            .connection
            .prepare("SELECT raw FROM body WHERE account = ?1 AND key = ?2")?;
        let mut keys = HashSet::new();
        let mut kept = Vec::new();
        for (key, mut message) in rows {
            if keys.insert(key) {
                message.raw = body
                    .query_row(params![account, key], |row| row.get(0))
                    .optional()?;
                kept.push(message);
            }
        }

        Ok(Some(kept))
    }
}

/// A row of the body key, then `THREAD_ROW`, as its Message without a body.
fn thread_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<(BodyKey, ThreadMessage)> {
    let offset = FixedOffset::east_opt(row.get(2)?).unwrap_or_else(|| Utc.fix());
    let received = DateTime::from_timestamp(row.get(1)?, 0)
        .unwrap_or_default()
        .with_timezone(&offset);
    let kept = ThreadMessage {
        from: row.get(3)?,
        to: row.get(4)?,
        received,
        raw: None,
    };
    Ok((row.get(0)?, kept))
}
