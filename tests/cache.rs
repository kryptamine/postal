#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    time::{Duration, Instant},
};

use support::{
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage, Stall},
    home::Home,
    picker::wait_picked,
};
use termlens::{Color, Key, Screen, Signal, Terminal};

const ITEMS: &str = "(UID FLAGS INTERNALDATE ENVELOPE X-GM-MSGID X-GM-THRID BODYSTRUCTURE BODY.PEEK[HEADER.FIELDS (FROM)])";

fn postal(home: &Home) -> Terminal {
    Terminal::builder()
        .size(80, 38)
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
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

/// Runs Postal until the server's rows are on screen, then quits.
fn first_run(home: &Home, server: &FakeImap) {
    let mut t = postal(home);
    t.wait_until(|s| s.contains(" · 1/")).unwrap();
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    quit(t);
    server.clear_commands();
}

fn wait_for(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// UIDs 101 to 104, the third Unread.
fn four_messages() -> Vec<FakeMessage> {
    vec![
        FakeMessage::new("Alice", "Second newest", "2024-03-02T09:00:00Z"),
        FakeMessage::new("Bob", "Oldest", "2023-12-31T23:59:00Z"),
        FakeMessage::new("Carol", "Newest", "2024-05-10T08:30:00Z").unread(),
        FakeMessage::new("Dave", "Third", "2024-01-15T12:00:00Z"),
    ]
}

fn hint_line(screen: &Screen) -> String {
    screen.row_text(screen.rows() - 1).trim_end().to_owned()
}

/// Whether the card naming `sender` shows the Unread marker; `None` when
/// no card on screen names it.
fn card(screen: &Screen, sender: &str) -> Option<bool> {
    let row = (0..screen.rows()).find(|&row| screen.row_text(row).contains(sender))?;
    Some(screen.cell(row, 2)?.contents() == "●")
}

#[test]
fn a_second_run_shows_the_cache_before_the_server_answers() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    first_run(&home, &server);

    server.stall(Stall::Greeting);
    let mut t = postal(&home);
    let screen = t
        .snapshot_after(|s| s.contains("Oldest") && s.contains(" Inbox · 1/4 "))
        .unwrap();
    assert_eq!(card(&screen, "Carol"), Some(true), "{screen}");
    wait_picked(&mut t, "Inbox 1 / 4");
    assert!(server.commands().is_empty(), "{:#?}", server.commands());
    quit(t);
}

#[test]
fn resync_picks_up_new_expunged_and_flag_changed_messages() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    first_run(&home, &server);

    let new =
        server.deliver(FakeMessage::new("Erin", "Brand new", "2024-06-01T00:00:00Z").unread());
    server.expunge(FIRST_UID + 1);
    server.set_seen(FIRST_UID + 2, true);
    server.set_seen(FIRST_UID, false);
    let mut t = postal(&home);
    let screen = t
        .snapshot_after(|s| s.contains("Brand new") && !s.contains("Oldest"))
        .unwrap();
    wait_picked(&mut t, "Inbox 2 / 4");
    quit(t);

    assert_eq!(card(&screen, "Erin"), Some(true), "{screen}");
    assert_eq!(card(&screen, "Carol"), Some(false), "{screen}");
    assert_eq!(card(&screen, "Alice"), Some(true), "{screen}");
    let commands = server.commands();
    for expected in [
        format!("UID FETCH {new}:* {ITEMS}"),
        "UID SEARCH UID 101:104".to_owned(),
        "UID FETCH 101:104 (UID FLAGS) (CHANGEDSINCE 5)".to_owned(),
    ] {
        assert!(commands.contains(&expected), "{expected}: {commands:#?}");
    }
}

#[test]
fn an_unchanged_inbox_is_checked_without_listing_it() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    first_run(&home, &server);

    let t = postal(&home);
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    quit(t);

    let commands = server.commands();
    assert!(
        commands.contains(&"UID FETCH 101 UID".to_owned()),
        "{commands:#?}"
    );
    // Counts that match prove nothing was expunged; equal mod-sequences, no flag changed.
    for skipped in [
        "UID SEARCH UID",
        "CHANGEDSINCE",
        "UID FETCH 105:*",
        "FETCH 1:*",
    ] {
        assert!(
            !commands.iter().any(|c| c.contains(skipped)),
            "{skipped}: {commands:#?}"
        );
    }
}

#[test]
fn an_expunge_older_than_the_cache_needs_no_search() {
    let messages = (0..503)
        .map(|i| {
            let date = chrono::DateTime::UNIX_EPOCH + chrono::Duration::hours(i);
            FakeMessage::new("Sender", &format!("Message {}", i + 1), &date.to_rfc3339())
        })
        .collect();
    let server = FakeImap::start(messages);
    let home = Home::for_server(&server);
    first_run(&home, &server);

    // The Cache holds UIDs 104 to 603; 101 to 103 are older than its window.
    server.expunge(FIRST_UID);
    let t = postal(&home);
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    quit(t);
    assert!(
        !server
            .commands()
            .iter()
            .any(|c| c.starts_with("UID SEARCH UID")),
        "{:#?}",
        server.commands()
    );

    server.clear_commands();
    server.expunge(FIRST_UID + 4);
    let t = postal(&home);
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    quit(t);
    assert!(
        server
            .commands()
            .contains(&"UID SEARCH UID 104:603".to_owned()),
        "{:#?}",
        server.commands()
    );
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    let gone: i64 = cache
        .query_row("SELECT count(*) FROM message WHERE uid = 105", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(gone, 0);
}

#[test]
fn flags_are_fetched_in_full_without_condstore() {
    let server = FakeImap::start(four_messages());
    server.without_condstore();
    let home = Home::for_server(&server);
    first_run(&home, &server);

    server.set_seen(FIRST_UID + 2, true);
    let mut t = postal(&home);
    t.wait_until(|s| card(s, "Carol") == Some(false)).unwrap();
    wait_picked(&mut t, "Inbox 4");
    quit(t);
    assert!(
        server
            .commands()
            .contains(&"UID FETCH 101:104 (UID FLAGS)".to_owned()),
        "{:#?}",
        server.commands()
    );
}

#[test]
fn a_uidvalidity_change_keeps_the_old_rows_until_the_refetch() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    first_run(&home, &server);

    server.renumber(vec![FakeMessage::new(
        "Zed",
        "Renumbered",
        "2024-07-01T00:00:00Z",
    )]);
    server.stall(Stall::Fetch);
    let mut t = postal(&home);
    let fetch = format!("FETCH 1:* {ITEMS}");
    wait_for(|| server.commands().contains(&fetch));
    let waiting = t.wait_stable(Duration::from_millis(200)).unwrap();
    assert!(waiting.contains("Newest"), "{waiting}");

    server.release();
    t.wait_until(|s| s.contains("Renumbered") && !s.contains("Newest"))
        .unwrap();
    wait_picked(&mut t, "Inbox 1");
    quit(t);

    // The next run resyncs against the new UIDVALIDITY rather than refetching.
    server.clear_commands();
    let t = postal(&home);
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    quit(t);
    assert!(
        !server.commands().contains(&fetch),
        "{:#?}",
        server.commands()
    );
}

#[test]
fn a_corrupt_cache_is_rebuilt() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    fs::create_dir_all(home.cache_path().parent().unwrap()).unwrap();
    fs::write(home.cache_path(), "not a database, just bytes ".repeat(100)).unwrap();
    let mut t = postal(&home);
    t.wait_until(|s| s.contains(" · 1/") && hint_line(s).ends_with("cache rebuilt"))
        .unwrap();
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    quit(t);
    let log = fs::read_to_string(home.log_path()).unwrap();
    assert!(log.contains("cache rebuilt"), "{log}");

    // A Cache that opens as it is says nothing.
    let mut t = postal(&home);
    t.wait_until(|s| s.contains(" · 1/")).unwrap();
    let screen = t.wait_stable(Duration::from_millis(300)).unwrap();
    assert!(!hint_line(&screen).contains("cache rebuilt"), "{screen}");
    quit(t);

    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    let rows: i64 = cache
        .query_row("SELECT count(*) FROM message", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 4);
}

// The check of the whole file costs a start 0.8 s at 80 MB, so only a
// session that did not end in order is followed by it.
#[test]
fn a_cache_damaged_under_a_killed_session_is_checked_and_rebuilt() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    first_run(&home, &server);
    first_run(&home, &server);
    let log = fs::read_to_string(home.log_path()).unwrap();
    assert!(!log.contains("checking the Cache"), "{log}");

    let mut t = postal(&home);
    t.wait_until(|s| s.contains(" · 1/")).unwrap();
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    t.signal(Signal::Kill).unwrap();
    t.wait_exit().unwrap();
    // The schema's pages stay, so only the check finds the damage.
    damage(&home, "message");

    let mut t = postal(&home);
    t.wait_until(|s| s.contains(" · 1/") && hint_line(s).ends_with("cache rebuilt"))
        .unwrap();
    quit(t);
    let log = fs::read_to_string(home.log_path()).unwrap();
    assert!(log.contains("checking the Cache"), "{log}");
}

/// Overwrites the root page of `table` in the closed Cache with garbage.
fn damage(home: &Home, table: &str) {
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    let (root, size): (usize, usize) = cache
        .query_row(
            "SELECT rootpage, (SELECT page_size FROM pragma_page_size) \
             FROM sqlite_master WHERE name = ?1",
            [table],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    drop(cache);
    let mut bytes = fs::read(home.cache_path()).unwrap();
    bytes[(root - 1) * size..root * size].fill(0xff);
    fs::write(home.cache_path(), bytes).unwrap();
}

#[test]
fn a_schema_from_before_migrations_is_rebuilt() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    fs::create_dir_all(home.cache_path().parent().unwrap()).unwrap();
    let old = rusqlite::Connection::open(home.cache_path()).unwrap();
    old.execute_batch("CREATE TABLE rows (x); PRAGMA user_version = 17;")
        .unwrap();
    drop(old);
    first_run(&home, &server);

    let log = fs::read_to_string(home.log_path()).unwrap();
    assert!(log.contains("cache rebuilt: schema 17"), "{log}");
}

#[test]
fn a_newer_schema_exits_before_the_tui_and_is_left_alone() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    fs::create_dir_all(home.cache_path().parent().unwrap()).unwrap();
    let newer = rusqlite::Connection::open(home.cache_path()).unwrap();
    newer
        .execute_batch("CREATE TABLE future (x); PRAGMA user_version = 99;")
        .unwrap();
    drop(newer);
    let before = fs::read(home.cache_path()).unwrap();

    let output = home.command(env!("CARGO_BIN_EXE_postal")).output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let dir = home.xdg_cache_home().join("postal");
    let expected = format!(
        "Cache was written by a newer Postal (schema 99 > 33). Upgrade Postal or delete {}.",
        dir.display()
    );
    assert!(stderr.contains(&expected), "{stderr}");
    assert_eq!(fs::read(home.cache_path()).unwrap(), before);
    assert!(server.commands().is_empty(), "{:#?}", server.commands());
}

fn set_mode(dir: &Path, mode: u32) {
    fs::set_permissions(dir, fs::Permissions::from_mode(mode)).unwrap();
}

// Losing the Cache directory mid-session must not end Postal or freeze the
// Message List: new mail and its body still show.
#[test]
fn a_read_only_cache_directory_mid_session_keeps_the_ui_working() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Newest")).unwrap();
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));

    let dir = home.cache_path().parent().unwrap().to_owned();
    set_mode(&dir, 0o555);
    server.deliver(
        FakeMessage::new("Erin", "Brand new", "2024-06-01T00:00:00Z")
            .body(FakeBody::plain("Fresh body")),
    );
    t.wait_until(|s| s.contains("Brand new")).unwrap();
    t.send(Key::Char('k')).unwrap();
    t.wait_until(|s| s.contains(" 1/5 ") && preview(s).contains("Fresh body"))
        .unwrap();
    set_mode(&dir, 0o755);
    quit(t);
}

/// The Preview's text at 80 columns, 33 rows: the rows below the list.
fn preview(screen: &Screen) -> String {
    (15..screen.rows() - 1)
        .map(|row| screen.row_text(row))
        .collect()
}

fn toast_is_red(screen: &Screen, text: &str) -> bool {
    let line = hint_line(screen);
    let Some((before, _)) = line.split_once(text) else {
        return false;
    };

    let column = u16::try_from(before.chars().count()).unwrap();
    screen
        .cell(screen.rows() - 1, column)
        .is_some_and(|cell| cell.style().fg == Color::Indexed(1))
}

// A write the disk refuses moves the Cache into memory: the session goes on
// with new mail and bodies, and the failure is told once, not on every write.
#[test]
fn a_failed_cache_write_keeps_the_session_in_memory_and_is_toasted_once() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Newest")).unwrap();
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    // The body fetch and the Dwell write too; one the lock caught would wait
    // out SQLite's busy timeout and leave too little of the timeout below.
    t.wait_until(|s| card(s, "Carol") == Some(false) && !preview(s).contains("Loading…"))
        .unwrap();
    wait_for(|| server.commands().iter().any(|c| c.contains("STORE")));

    // Another writer holding the lock makes every write of Postal's fail.
    let lock = rusqlite::Connection::open(home.cache_path()).unwrap();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    server.deliver(
        FakeMessage::new("Erin", "Brand new", "2024-06-01T00:00:00Z")
            .body(FakeBody::plain("Fresh body")),
    );
    let screen = t
        .snapshot_after(|s| s.contains("Brand new") && hint_line(s).ends_with("cache write failed"))
        .unwrap();
    assert!(toast_is_red(&screen, "cache write failed"), "{screen}");

    t.send(Key::Char('k')).unwrap();
    t.wait_until(|s| s.contains(" 1/5 ") && preview(s).contains("Fresh body"))
        .unwrap();
    t.wait_until(|s| !hint_line(s).contains("cache write failed"))
        .unwrap();
    server.deliver(FakeMessage::new(
        "Finn",
        "Newer still",
        "2024-07-01T00:00:00Z",
    ));
    t.wait_until(|s| s.contains("Newer still")).unwrap();
    let screen = t.wait_stable(Duration::from_millis(300)).unwrap();
    assert!(
        !hint_line(&screen).contains("cache write failed"),
        "{screen}"
    );
    quit(t);

    lock.execute_batch("ROLLBACK").unwrap();
    let log = fs::read_to_string(home.log_path()).unwrap();
    assert!(
        log.lines()
            .any(|l| l.contains("ERROR") && l.contains("cache write failed")),
        "{log}"
    );
    let on_disk: i64 = lock
        .query_row("SELECT count(*) FROM message", [], |row| row.get(0))
        .unwrap();
    assert_eq!(on_disk, 4);
}

// A read the Cache fails in answer to a key must not end Postal: the error
// is toasted and the screen stays usable.
#[test]
fn a_failed_cache_read_is_toasted_and_the_session_goes_on() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    first_run(&home, &server);
    damage(&home, "message_text_data");

    // Synced mail would write to the damaged index before the Search.
    server.stall(Stall::Greeting);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains(" Inbox · 1/4 ")).unwrap();
    t.send(Key::Char('/')).unwrap();
    t.send_str("old").unwrap();
    let screen = t
        .snapshot_after(|s| hint_line(s).ends_with("cache error"))
        .unwrap();
    assert!(toast_is_red(&screen, "cache error"), "{screen}");
    t.send(Key::Esc).unwrap();
    t.wait_until(|s| !hint_line(s).contains("/old")).unwrap();
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains(" Inbox · 2/4 ")).unwrap();
    quit(t);

    let log = fs::read_to_string(home.log_path()).unwrap();
    assert!(log.contains("malformed"), "{log}");
}
