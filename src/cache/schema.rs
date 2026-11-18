use std::{fs, io, path::Path};

use rusqlite::Connection;

use super::Error;

/// Bumped with every change to `TABLES`; an older Cache has no migration and is rebuilt.
pub const VERSION: i64 = 3;

const FILE: &str = "cache.sqlite3";

const TABLES: &str = "
CREATE TABLE mailbox (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    uid_validity INTEGER NOT NULL,
    uid_next INTEGER,
    highest_modseq INTEGER,
    unread INTEGER NOT NULL
);
CREATE TABLE message (
    uid INTEGER PRIMARY KEY,
    -- The `body` row of this Message, once fetched: see docs/adr/0005-body-cache-key.md.
    body_key INTEGER NOT NULL,
    received INTEGER NOT NULL,
    received_offset INTEGER NOT NULL,
    unread INTEGER NOT NULL,
    sender TEXT NOT NULL,
    subject TEXT NOT NULL,
    -- NULL until the second fetch phase writes it.
    snippet TEXT,
    -- The part the Snippet and the Preview show; NULL with no text part.
    text_section TEXT,
    text_encoding TEXT,
    text_charset TEXT,
    text_html INTEGER
);
CREATE INDEX message_by_arrival ON message (received DESC, uid DESC);
-- Not dropped with the rows on a UIDVALIDITY change; a byte-bounded LRU.
CREATE TABLE body (
    key INTEGER PRIMARY KEY,
    raw BLOB NOT NULL,
    size INTEGER NOT NULL,
    -- Bumped on every write and read; the lowest goes first.
    used INTEGER NOT NULL
);
CREATE INDEX body_by_use ON body (used);
";

/// Why an existing Cache file cannot be used as it is.
enum Unusable {
    Missing,
    Newer(i64),
    Stale(String),
}

/// Opens the Cache in `dir`. A corrupt or older one is deleted and rebuilt;
/// a newer one is left untouched, since rebuilding it would churn the Cache
/// on every upgrade and downgrade.
pub fn open(dir: &Path) -> Result<Connection, Error> {
    fs::create_dir_all(dir)?;
    let path = dir.join(FILE);
    match existing(&path) {
        Ok(connection) => return Ok(connection),
        Err(Unusable::Newer(found)) => {
            return Err(Error::NewerSchema {
                found,
                dir: dir.to_owned(),
            });
        }
        Err(Unusable::Missing) => tracing::info!("creating the Cache at {}", path.display()),
        Err(Unusable::Stale(reason)) => tracing::warn!("cache rebuilt: {reason}"),
    }

    for suffix in ["", "-wal", "-shm"] {
        let mut file = path.clone().into_os_string();
        file.push(suffix);
        match fs::remove_file(file) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
    }

    let connection = Connection::open(&path)?;
    configure(&connection)?;
    connection.execute_batch(TABLES)?;
    connection.pragma_update(None, "user_version", VERSION)?;
    Ok(connection)
}

fn existing(path: &Path) -> Result<Connection, Unusable> {
    if !path.exists() {
        return Err(Unusable::Missing);
    }

    let stale = |error: rusqlite::Error| Unusable::Stale(error.to_string());
    let connection = Connection::open(path).map_err(stale)?;
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(stale)?;
    if version > VERSION {
        return Err(Unusable::Newer(version));
    }

    if version < VERSION {
        return Err(Unusable::Stale(format!(
            "schema {version} has no migration to {VERSION}"
        )));
    }

    configure(&connection).map_err(stale)?;
    let check: String = connection
        .pragma_query_value(None, "quick_check", |row| row.get(0))
        .map_err(stale)?;
    if check != "ok" {
        return Err(Unusable::Stale(check));
    }

    Ok(connection)
}

/// A commit lost to a crash under `synchronous=NORMAL` is only refetched.
fn configure(connection: &Connection) -> rusqlite::Result<()> {
    connection.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
    connection.pragma_update(None, "synchronous", "NORMAL")
}
