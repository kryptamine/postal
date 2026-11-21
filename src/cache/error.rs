use std::{io, path::PathBuf};

use super::schema::VERSION;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cache: {0}")]
    Io(#[from] io::Error),
    #[error("cache: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error(
        "Cache was written by a newer Postal (schema {found} > {VERSION}). \
         Upgrade Postal or delete {}.",
        .dir.display()
    )]
    NewerSchema { found: i64, dir: PathBuf },
    /// Read-only, there is no Cache of this schema to read.
    #[error("Cache not ready: run `postal` once")]
    NotReady,
}
