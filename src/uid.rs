use std::fmt;

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};

/// A Message's IMAP UID, which names it in one Folder under one
/// UIDVALIDITY.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Uid(u32);

impl Uid {
    pub const fn new(uid: u32) -> Self {
        Self(uid)
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    /// The lowest UID a Message newer than this one can have.
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// As the bare number, which logs have always shown.
impl fmt::Debug for Uid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for Uid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl ToSql for Uid {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.0.into())
    }
}

impl FromSql for Uid {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        u32::column_result(value).map(Self)
    }
}
