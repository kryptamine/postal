use std::{collections::BTreeSet, fs, io, path::Path};

use rusqlite::{Connection, OpenFlags};

use super::Error;

/// Bumped with every change to `TABLES`; an older Cache has no migration and is rebuilt.
pub const VERSION: i64 = 17;

const FILE: &str = "cache.sqlite3";

/// The columns of a `message` row but its Folder and UID, which `moved`
/// keeps too; `open` checks both tables against it in a debug build.
pub const MOVED_ROW: &str = "account, body_key, received, received_offset, unread, answered,
    sender, from_line, to_line, subject, thread, from_me, snippet, text_section, text_encoding,
    text_charset, text_html";

const TABLES: &str = "
-- Every row names its Account by address, lowercased.
-- One row per Folder, from `LIST` or a resync; the sync columns are NULL
-- until the Folder is first resynced.
CREATE TABLE mailbox (
    account TEXT NOT NULL,
    name TEXT NOT NULL,
    -- Its special use (RFC 6154), as `Role::label` names it; NULL for none.
    role TEXT,
    uid_validity INTEGER,
    uid_next INTEGER,
    highest_modseq INTEGER,
    -- The server's `EXISTS` as the last resync or IDLE update saw it.
    exists_count INTEGER,
    -- NULL until a resync or a `STATUS` counts it.
    unread INTEGER,
    PRIMARY KEY (account, name)
);
CREATE TABLE message (
    account TEXT NOT NULL,
    mailbox TEXT NOT NULL,
    uid INTEGER NOT NULL,
    -- The `body` row of this Message, once fetched: see docs/adr/0005-body-cache-key.md.
    body_key INTEGER NOT NULL,
    received INTEGER NOT NULL,
    received_offset INTEGER NOT NULL,
    unread INTEGER NOT NULL,
    answered INTEGER NOT NULL,
    sender TEXT NOT NULL,
    from_line TEXT NOT NULL,
    to_line TEXT NOT NULL,
    subject TEXT NOT NULL,
    -- Gmail's `X-GM-THRID`; NULL on another server.
    thread INTEGER,
    from_me INTEGER NOT NULL,
    -- NULL until the second fetch phase writes it.
    snippet TEXT,
    -- The part the Snippet and the Preview show; NULL with no text part.
    text_section TEXT,
    text_encoding TEXT,
    text_charset TEXT,
    text_html INTEGER,
    PRIMARY KEY (account, mailbox, uid)
);
CREATE INDEX message_by_arrival ON message (account, mailbox, received DESC, uid DESC);
-- A Thread's Messages as `[Gmail]/All Mail` held them when it was last read;
-- see docs/adr/0009-threads-from-gmail-thread-id.md.
CREATE TABLE thread_message (
    account TEXT NOT NULL,
    -- Its `X-GM-MSGID`, the key its body is cached under.
    key INTEGER NOT NULL,
    thread INTEGER NOT NULL,
    received INTEGER NOT NULL,
    received_offset INTEGER NOT NULL,
    unread INTEGER NOT NULL,
    answered INTEGER NOT NULL,
    sender TEXT NOT NULL,
    from_line TEXT NOT NULL,
    to_line TEXT NOT NULL,
    subject TEXT NOT NULL,
    from_me INTEGER NOT NULL,
    snippet TEXT NOT NULL,
    PRIMARY KEY (account, key)
);
CREATE INDEX thread_message_by_thread ON thread_message (account, thread);
-- Shared by every Folder, and not dropped with the rows on a UIDVALIDITY
-- change; a byte-bounded LRU.
CREATE TABLE body (
    account TEXT NOT NULL,
    key INTEGER NOT NULL,
    raw BLOB NOT NULL,
    size INTEGER NOT NULL,
    -- Bumped on every write and read; the lowest goes first.
    used INTEGER NOT NULL,
    PRIMARY KEY (account, key)
);
CREATE INDEX body_by_use ON body (used);
-- A flag change the server has not acknowledged yet; a resync leaves that
-- flag of the row alone. See docs/adr/0011-select-the-shown-folder.md.
CREATE TABLE flag_change (
    account TEXT NOT NULL,
    mailbox TEXT NOT NULL,
    uid INTEGER NOT NULL,
    -- As IMAP names it: `\\Seen` or `\\Answered`.
    flag TEXT NOT NULL,
    added INTEGER NOT NULL,
    PRIMARY KEY (account, mailbox, uid, flag)
);
-- A Message a Delete took out of `message`, with its row, while the
-- server or an Undo may still move it: docs/adr/0012-delete-and-undo.md.
CREATE TABLE moved (
    id INTEGER PRIMARY KEY,
    -- One per Delete, shared by its Messages.
    batch INTEGER NOT NULL,
    -- Whether the Message is wanted in Trash and whether the server has it
    -- there, under `at_uid` there or in `mailbox`; NULL when never said.
    want_trash INTEGER NOT NULL,
    at_trash INTEGER NOT NULL,
    at_uid INTEGER,
    trash TEXT,
    account TEXT NOT NULL,
    mailbox TEXT NOT NULL,
    uid INTEGER NOT NULL,
    body_key INTEGER NOT NULL,
    received INTEGER NOT NULL,
    received_offset INTEGER NOT NULL,
    unread INTEGER NOT NULL,
    answered INTEGER NOT NULL,
    sender TEXT NOT NULL,
    from_line TEXT NOT NULL,
    to_line TEXT NOT NULL,
    subject TEXT NOT NULL,
    thread INTEGER,
    from_me INTEGER NOT NULL,
    snippet TEXT,
    text_section TEXT,
    text_encoding TEXT,
    text_charset TEXT,
    text_html INTEGER
);
-- What the server holds in `mailbox` that the Message List leaves out.
-- An address a Message names, once per Message whichever Folders hold it;
-- none from Trash or Spam: docs/adr/0016-expand-recipients-after-the-editor.md.
CREATE TABLE correspondence (
    account TEXT NOT NULL,
    -- Lowercased.
    address TEXT NOT NULL,
    -- Its `Message-ID`, else a key of its own.
    message TEXT NOT NULL,
    name TEXT,
    -- Whether the Account wrote to it, rather than had mail from or with it.
    written INTEGER NOT NULL,
    at INTEGER NOT NULL,
    PRIMARY KEY (account, address, message)
);
-- How far the pass over the Folder with the Sent role has read.
CREATE TABLE sent_pass (
    account TEXT PRIMARY KEY,
    mailbox TEXT NOT NULL,
    uid_validity INTEGER NOT NULL,
    last_uid INTEGER NOT NULL
);
CREATE VIEW deleted AS
    SELECT account, mailbox, at_uid AS uid FROM moved WHERE want_trash AND NOT at_trash;
-- Rows an Undo put back under `uid`, with where the server has the Message.
CREATE VIEW undone AS
    SELECT account, mailbox, uid, at_trash, at_uid FROM moved WHERE NOT want_trash;
";

/// Why an existing Cache file cannot be used as it is.
enum Unusable {
    Missing,
    Newer(i64),
    Stale(String),
}

/// Opens the Cache in `dir`. A corrupt or older one is deleted and rebuilt;
/// a newer one is left untouched, since rebuilding it would churn the Cache
/// on every upgrade and downgrade. Says whether it replaced one.
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
        Ok(connection) => return Ok((connection, false)),
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
    let connection = Connection::open(&path)?;
    configure(&connection)?;
    connection.execute_batch(TABLES)?;
    connection.pragma_update(None, "user_version", VERSION)?;
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
