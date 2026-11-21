use std::fmt;

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};

/// Gmail's `X-GM-THRID`, which names a Thread across the Folders of one
/// Account.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThreadId(u64);

impl ThreadId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
}

impl fmt::Display for ThreadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// `SQLite` has no unsigned 64-bit integer: the bits are kept as an `i64`.
impl ToSql for ThreadId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.0.cast_signed().into())
    }
}

impl FromSql for ThreadId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(|id| Self(id.cast_unsigned()))
    }
}
