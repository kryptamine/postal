use std::{collections::BTreeSet, fs, io, path::Path};

use rusqlite::{Connection, OpenFlags};

use super::{
    Error,
    migrations::{self, PRE_RELEASE, VERSION},
};

const FILE: &str = "cache.sqlite3";

/// The columns of a `message` row but its Folder and UID, which `moved`
/// keeps too; `open` checks both tables against it in a debug build.
pub const MOVED_ROW: &str = "account, body_key, received, received_offset, unread, answered,
    sender, from_line, to_line, subject, thread, from_me, snippet, text_section, text_encoding,
    text_charset, text_html";

/// Why an existing Cache file cannot be used as it is.
enum Unusable {
    Missing,
    Newer(i64),
    Stale(String),
}

/// Opens the Cache in `dir`, migrated to this schema. A corrupt one, or one
/// from before migrations, is deleted and rebuilt; a newer one is left
/// untouched for the Postal that wrote it. Says whether it replaced one.
pub fn open(dir: &Path) -> Result<(Connection, bool), Error> {
    let opened = open_unchecked(dir)?;
    debug_assert!(
        matches!(rows_match(&opened.0), Ok(true)),
        "`message` and `moved` disagree with MOVED_ROW"
    );
    Ok(opened)
}

fn open_unchecked(dir: &Path) -> Result<(Connection, bool), Error> {
    fs::create_dir_all(dir)?;
    let path = dir.join(FILE);
    let rebuilt = match existing(&path) {
        Ok((mut connection, version)) => {
            migrations::migrate(&mut connection, version)?;
            return Ok((connection, false));
        }

        Err(Unusable::Newer(found)) => {
            return Err(Error::NewerSchema {
                found,
                dir: dir.to_owned(),
            });
        }
        Err(Unusable::Missing) => {
            tracing::info!("creating the Cache at {}", path.display());
            false
        }
        Err(Unusable::Stale(reason)) => {
            tracing::warn!("cache rebuilt: {reason}");
            true
        }
    };

    remove(&path)?;
    let mut connection = Connection::open(&path)?;
    configure(&connection)?;
    migrations::migrate(&mut connection, 0)?;
    Ok((connection, rebuilt))
}

/// Whether `message` holds `MOVED_ROW` with its Folder and UID, and `moved`
/// holds `MOVED_ROW` among its own columns.
fn rows_match(connection: &Connection) -> rusqlite::Result<bool> {
    let columns = |table: &str| -> rusqlite::Result<BTreeSet<String>> {
        connection
            .prepare("SELECT name FROM pragma_table_info(?1)")?
            .query_map([table], |row| row.get(0))?
            .collect()
    };
    let row: BTreeSet<String> = MOVED_ROW
        .split(',')
        .map(|name| name.trim().to_owned())
        .collect();
    let mut message = columns("message")?;
    let folder_and_uid = message.remove("mailbox") && message.remove("uid");
    Ok(folder_and_uid && message == row && columns("moved")?.is_superset(&row))
}

/// The Cache in `dir` for reading alone, as a TUI may have it open: never
/// created, rebuilt or written; `NotReady` unless it has this schema.
pub fn open_read_only(dir: &Path) -> Result<Connection, Error> {
    let path = dir.join(FILE);
    if !path.exists() {
        return Err(Error::NotReady);
    }

    let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version != VERSION {
        return Err(Error::NotReady);
    }

    Ok(connection)
}

/// Deletes the Cache in `dir`, if there is one.
pub fn delete(dir: &Path) -> Result<(), Error> {
    remove(&dir.join(FILE))
}

/// The database with its WAL files.
fn remove(path: &Path) -> Result<(), Error> {
    for suffix in ["", "-wal", "-shm"] {
        let mut file = path.as_os_str().to_owned();
        file.push(suffix);
        match fs::remove_file(file) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
    }

    Ok(())
}

/// The Cache at `path` with its `user_version`, once it checks out.
fn existing(path: &Path) -> Result<(Connection, i64), Unusable> {
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

    if version <= PRE_RELEASE {
        return Err(Unusable::Stale(format!(
            "schema {version} predates the first migration"
        )));
    }

    configure(&connection).map_err(stale)?;
    let check: String = connection
        .pragma_query_value(None, "quick_check", |row| row.get(0))
        .map_err(stale)?;
    if check != "ok" {
        return Err(Unusable::Stale(check));
    }

    Ok((connection, version))
}

/// A commit lost to a crash under `synchronous=NORMAL` is only refetched.
fn configure(connection: &Connection) -> rusqlite::Result<()> {
    connection.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
    connection.pragma_update(None, "synchronous", "NORMAL")
}
