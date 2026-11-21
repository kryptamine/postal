use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::Connection;

use super::{Error, schema};

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
}
