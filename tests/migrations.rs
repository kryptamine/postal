//! A Cache each released Postal wrote, opened by this one: what the server
//! has not seen yet survives the migration. See
//! docs/adr/0021-migrate-stored-data-across-releases.md.

use crate::support;

use std::{fmt::Write, fs, path::PathBuf, process::Command};

use support::{
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage, INBOX, Outage, Stall},
    home::Home,
    quit, wait_for,
};
use termlens::{Key, Screen, Terminal};

const TRASH: &str = "[Gmail]/Trash";

fn postal(home: &Home) -> Terminal {
    home.terminal()
        .size(120, 30)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn message(sender: &'static str, subject: &str, date: &str) -> FakeMessage {
    FakeMessage::new(sender, subject, date).body(FakeBody::plain(&format!("Body of {subject}")))
}

/// Alice's, Bob's Unread and Carol's in the Inbox, oldest first, beside an
/// empty Trash.
fn server() -> FakeImap {
    let server = FakeImap::start(vec![
        message("Alice", "Oldest", "2024-03-01T09:00:00Z"),
        message("Bob", "Middle", "2024-03-02T09:00:00Z").unread(),
        message("Carol", "Newest", "2024-03-03T09:00:00Z"),
    ]);
    server.add_folder(TRASH, &["\\Trash"], vec![]);
    server
}

/// Whether the card naming `sender` shows the Unread marker; `None` when
/// no card on screen names it.
fn card(screen: &Screen, sender: &str) -> Option<bool> {
    let row = (0..screen.rows()).find(|&row| screen.row_text(row).contains(sender))?;
    Some(screen.cell(row, 2)?.contents() == "●")
}

/// Named by `user_version`, not by the migration step.
fn fixture(version: i64) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/cache/{version}.sql"))
}

/// Carol's Message deleted and Bob's read while offline, neither sent yet.
fn pending(server: &FakeImap, home: &Home) {
    let mut t = postal(home);
    t.wait_until(|s| s.contains("Body of Newest")).unwrap();
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    quit(t);
    server.fail(Outage::Offline);

    let mut t = postal(home);
    t.wait_until(|s| s.contains("Body of Newest")).unwrap();
    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| !s.contains("Newest") && card(s, "Bob") == Some(false))
        .unwrap();
    quit(t);
    server.recover();
}

/// The Cache of `version` holds a Delete open to an Undo and a flag change
/// the server has not seen; this Postal sends both and keeps the List.
fn survives(version: i64) {
    let server = server();
    let home = Home::for_server(&server);
    fs::create_dir_all(home.cache_path().parent().unwrap()).unwrap();
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    cache
        .execute_batch(&fs::read_to_string(fixture(version)).unwrap())
        .unwrap();
    drop(cache);

    server.stall(Stall::Greeting);
    let mut t = postal(&home);
    let screen = t
        .snapshot_after(|s| s.contains("Body of Middle") && s.contains("Oldest"))
        .unwrap();
    assert!(!screen.contains("Newest"), "{screen}");
    assert_eq!(card(&screen, "Bob"), Some(false), "{screen}");

    server.release();
    wait_for(|| server.subjects(TRASH) == ["Newest"]);
    wait_for(|| server.seen(INBOX, FIRST_UID + 1));
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    let screen = t.snapshot_after(|_| true).unwrap();
    assert!(!screen.contains("Newest"), "{screen}");
    assert_eq!(card(&screen, "Bob"), Some(false), "{screen}");

    // The index is whole, for the operators too.
    t.send(Key::Char('/')).unwrap();
    t.send_str("from:alice").unwrap();
    t.wait_until(|s| s.contains("Body of Oldest") && !s.contains("Middle"))
        .unwrap();
    quit(t);
    let log = fs::read_to_string(home.log_path()).unwrap();
    assert!(!log.contains("cache rebuilt"), "{log}");
}

#[test]
fn a_cache_of_schema_29_survives() {
    survives(29);
}

#[test]
fn a_cache_of_schema_30_survives() {
    survives(30);
}

#[test]
fn a_cache_of_schema_31_survives() {
    survives(31);
}

#[test]
fn a_cache_of_schema_32_survives() {
    survives(32);
}

#[test]
fn a_cache_of_schema_33_survives() {
    survives(33);
}

// Not a check: writes the fixture of this Postal's schema, with `sqlite3`.
// Run it before appending a migration, and add a test of that fixture.
#[test]
#[ignore = "writes tests/fixtures/cache/<schema>.sql"]
fn write_the_fixture_of_this_schema() {
    let server = server();
    let home = Home::for_server(&server);
    pending(&server, &home);

    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    let version: i64 = cache
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    drop(cache);
    let dump = Command::new("sqlite3")
        .arg(home.cache_path())
        // The index is keyed by rowid, which a plain `.dump` renumbers.
        .arg(".dump --preserve-rowids")
        .output()
        .unwrap();
    assert!(dump.status.success());
    let path = fixture(version);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut sql = String::from_utf8(dump.stdout).unwrap();
    writeln!(sql, "PRAGMA user_version = {version};").unwrap();
    fs::write(path, sql).unwrap();
}
