use std::{fmt, io, path::PathBuf};

use super::schema::VERSION;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Sqlite(rusqlite::Error),
    NewerSchema { found: i64, dir: PathBuf },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(f, "cache: {source}"),
            Self::Sqlite(source) => write!(f, "cache: {source}"),
            Self::NewerSchema { found, dir } => write!(
                f,
                "Cache was written by a newer Postal (schema {found} > {VERSION}). \
                 Upgrade Postal or delete {}.",
                dir.display()
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(source: rusqlite::Error) -> Self {
        Self::Sqlite(source)
    }
}
