//! The domain's ids as `SQLite` values, so `rusqlite` is named only in the Cache.

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

use crate::{
    domain::body_key::BodyKey, domain::folder_name::FolderName, domain::root_id::RootId,
    domain::thread_id::ThreadId, domain::uid::Uid,
};

impl ToSql for Uid {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.get().into())
    }
}

impl FromSql for Uid {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        u32::column_result(value).map(Self::new)
    }
}

/// `SQLite` has no unsigned 64-bit integer: the bits are kept as an `i64`.
impl ToSql for ThreadId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.get().cast_signed().into())
    }
}

impl FromSql for ThreadId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(|id| Self::new(id.cast_unsigned()))
    }
}

/// Kept as an `i64`, as `ThreadId` is.
impl ToSql for RootId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.get().cast_signed().into())
    }
}

impl FromSql for RootId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(|id| Self::new(id.cast_unsigned()))
    }
}

impl ToSql for BodyKey {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.get().into())
    }
}

impl FromSql for BodyKey {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Self::from_stored)
    }
}

impl ToSql for FolderName {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for FolderName {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        String::column_result(value).map(Self::from)
    }
}
