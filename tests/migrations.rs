//! A Cache each released Postal wrote, opened by this one: what the server
//! has not seen yet survives the migration. See
//! docs/adr/0021-migrate-stored-data-across-releases.md.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{
    fmt::Write,
    fs,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

use support::{
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage, INBOX, Outage, Stall},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

const TRASH: &str = "[Gmail]/Trash";

fn postal(home: &Home) -> Terminal {
    Terminal::builder()
        .size(120, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn wait_for(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
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
        .arg(".dump")
        .output()
        .unwrap();
    assert!(dump.status.success());
    let path = fixture(version);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut sql = String::from_utf8(dump.stdout).unwrap();
    writeln!(sql, "PRAGMA user_version = {version};").unwrap();
    fs::write(path, sql).unwrap();
}
