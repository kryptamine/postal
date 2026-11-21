use std::fmt;

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

/// The key a Message's body is cached under: Gmail's `X-GM-MSGID`, which
/// survives a UIDVALIDITY change, else the UID under its UIDVALIDITY.
/// See docs/adr/0005-body-cache-key.md.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyKey(i64);

impl BodyKey {
    pub fn new(gmail_id: Option<u64>, uid_validity: u32, uid: u32) -> Self {
        Self(
            gmail_id
                .unwrap_or_else(|| (u64::from(uid_validity) << 32) | u64::from(uid))
                .cast_signed(),
        )
    }
}

impl fmt::Display for BodyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl ToSql for BodyKey {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for BodyKey {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Self)
    }
}
