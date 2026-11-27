#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::time::Duration;

use support::{
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage, Outage, Stall},
    home::Home,
    quit, wait_for,
};
use termlens::{Key, Screen, Terminal};

const ITEMS: &str = "(UID FLAGS INTERNALDATE ENVELOPE X-GM-MSGID X-GM-THRID BODYSTRUCTURE BODY.PEEK[HEADER.FIELDS (FROM)])";

fn postal(home: &Home) -> Terminal {
    home.terminal()
        .size(120, 30)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

/// Message 1 is the oldest, by sequence number and by date alike.
fn messages(count: i64) -> Vec<FakeMessage> {
    (1..=count)
        .map(|n| {
            let date = chrono::DateTime::UNIX_EPOCH + chrono::Duration::hours(n);
            let body = FakeBody::plain(&format!("Body of message {n}"));
            FakeMessage::new("Sender", &format!("Message {n}"), &date.to_rfc3339()).body(body)
        })
        .collect()
}

fn hint_line(screen: &Screen) -> String {
    screen.row_text(screen.rows() - 1)
}

/// The Preview's title: the selection's place in the Message List.
fn place(t: &mut Terminal, place: &str) {
    t.wait_until(|s| s.row_text(0).contains(&format!(" {place} ")))
        .unwrap();
}

fn sequence_fetches(server: &FakeImap) -> Vec<String> {
    let commands = server.commands();
    commands
        .into_iter()
        .filter(|c| c.starts_with("FETCH "))
        .collect()
}

fn cached(home: &Home, sql: &str) -> i64 {
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    cache.query_row(sql, [], |row| row.get(0)).unwrap()
}

/// Goes to the end of the Message List page by page, until it ends at
/// Message 1, the oldest of `count`.
fn scroll_to_the_oldest(t: &mut Terminal, count: usize) {
    let mut shown = 500;
    while shown < count {
        t.send(Key::Char('G')).unwrap();
        // The selection stays on the same Message as the page comes in.
        let next = (shown + 500).min(count);
        place(t, &format!("{shown}/{next}"));
        shown = next;
    }

    t.send(Key::Char('G')).unwrap();
    place(t, &format!("{count}/{count}"));
    t.wait_until(|s| s.contains("Subject Message 1\u{20}"))
        .unwrap();
}

// Reaching the end of the Message List brings the next 500 older Messages,
// oldest page last, each Message once, with its Snippet.
#[test]
fn scrolling_to_the_end_loads_the_older_pages_in_order() {
    let server = FakeImap::start(messages(1200));
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    place(&mut t, "1/500");
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));

    scroll_to_the_oldest(&mut t, 1200);
    t.wait_until(|s| s.contains("Body of message 1\u{20}"))
        .unwrap();
    wait_for(|| cached(&home, "SELECT count(*) FROM message WHERE snippet IS NULL") == 0);
    quit(t);

    assert_eq!(
        sequence_fetches(&server),
        [
            format!("FETCH 701:* {ITEMS}"),
            format!("FETCH 201:700 {ITEMS}"),
            format!("FETCH 1:200 {ITEMS}"),
        ]
    );
    assert_eq!(cached(&home, "SELECT count(*) FROM message"), 1200);
    assert_eq!(
        cached(&home, "SELECT count(DISTINCT subject) FROM message"),
        1200
    );
}

// The status slot counts a page of older Messages as it comes in.
#[test]
fn a_page_under_way_shows_its_progress() {
    let server = FakeImap::start(messages(700));
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    place(&mut t, "1/500");
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));

    server.stall(Stall::Sequence);
    t.send(Key::Char('G')).unwrap();
    t.wait_until(|s| hint_line(s).contains("syncing 0/200"))
        .unwrap();
    server.release();
    place(&mut t, "500/700");
    t.wait_until(|s| !hint_line(s).contains("syncing")).unwrap();
    quit(t);
}

// Backfilled rows are in the Cache: the next run shows them before the
// server answers, and its resync keeps them.
#[test]
fn backfilled_rows_survive_a_restart() {
    let server = FakeImap::start(messages(1200));
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    place(&mut t, "1/500");
    scroll_to_the_oldest(&mut t, 1200);
    quit(t);

    server.stall(Stall::Greeting);
    server.clear_commands();
    let mut t = postal(&home);
    place(&mut t, "1/1200");
    t.send(Key::Char('G')).unwrap();
    place(&mut t, "1200/1200");
    assert!(server.commands().is_empty(), "{:#?}", server.commands());

    server.release();
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    quit(t);
    assert_eq!(cached(&home, "SELECT count(*) FROM message"), 1200);
    // The resync covers the backfilled range: no window is fetched again.
    assert!(
        sequence_fetches(&server).is_empty(),
        "{:#?}",
        server.commands()
    );
}

// An expunge announced by IDLE reaches a backfilled row too, though the
// IDLE connection numbered the Messages before it came.
#[test]
fn idle_removes_an_expunged_backfilled_row() {
    let server = FakeImap::start(messages(700));
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    place(&mut t, "1/500");
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    scroll_to_the_oldest(&mut t, 700);

    // Message 10 is in the backfilled page, below the first window.
    server.expunge(FIRST_UID + 9);
    place(&mut t, "699/699");
    let subject = "SELECT count(*) FROM message WHERE subject = 'Message 10'";
    wait_for(|| cached(&home, subject) == 0);
    assert_eq!(cached(&home, "SELECT count(*) FROM message"), 699);
    quit(t);
}

// Another connection's expunge between a page's numbering and its fetch
// made the server refuse the numbers, and Postal quit on the refusal.
#[test]
fn a_page_renumbered_by_an_expunge_is_fetched_again() {
    let server = FakeImap::start(messages(700));
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    place(&mut t, "1/500");
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));

    server.stall(Stall::Sequence);
    t.send(Key::Char('G')).unwrap();
    let page = format!("FETCH 1:200 {ITEMS}");
    wait_for(|| server.commands().contains(&page));
    // Message 201, the oldest cached, and Message 200 below it are left.
    server.expunge_range(FIRST_UID..=FIRST_UID + 198);
    server.expunge_range(FIRST_UID + 201..=FIRST_UID + 699);
    server.release();

    let below = "SELECT count(*) FROM message WHERE subject = 'Message 200'";
    wait_for(|| cached(&home, below) == 1);
    quit(t);
}

fn snippet_fetches(server: &FakeImap) -> usize {
    let commands = server.commands();
    commands
        .iter()
        .filter(|c| c.contains("BODY.PEEK[1]<"))
        .count()
}

// The selection's body goes between a page's batches of Snippets, so its
// Preview never waits on the whole page.
#[test]
fn a_preview_does_not_wait_for_a_pages_snippets() {
    let server = FakeImap::start(messages(700));
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    place(&mut t, "1/500");
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));

    let before = snippet_fetches(&server);
    server.stall(Stall::Part);
    t.send(Key::Char('G')).unwrap();
    place(&mut t, "500/700");
    wait_for(|| snippet_fetches(&server) > before);
    t.send(Key::Char('G')).unwrap();
    place(&mut t, "700/700");
    server.release();
    t.wait_until(|s| s.contains("Body of message 1\u{20}"))
        .unwrap();
    wait_for(|| cached(&home, "SELECT count(*) FROM message WHERE snippet IS NULL") == 0);
    quit(t);

    let commands = server.commands();
    let body = format!("UID FETCH {FIRST_UID} ");
    let body = commands
        .iter()
        .position(|c| c.starts_with(&body) && c.contains("BODY.PEEK[HEADER]"));
    let last_snippet = commands.iter().rposition(|c| c.contains("BODY.PEEK[1]<"));
    assert!(body.is_some() && body < last_snippet, "{commands:#?}");
}

// A Folder the Cache holds whole has nothing older to page in: asking
// anyway spent a round trip on every run.
#[test]
fn a_wholly_cached_inbox_asks_for_no_older_page() {
    let server = FakeImap::start(messages(3));
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    place(&mut t, "1/3");
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    t.send(Key::Char('G')).unwrap();
    place(&mut t, "3/3");
    std::thread::sleep(Duration::from_millis(300));
    quit(t);
    let lookup = format!("UID FETCH {FIRST_UID} UID");
    assert!(
        !server.commands().contains(&lookup),
        "{:#?}",
        server.commands()
    );
}

// A backfill in flight when the fetch connection drops resumes by itself
// once the connection is back.
#[test]
fn a_backfill_interrupted_by_an_outage_resumes_after_reconnect() {
    let server = FakeImap::start(messages(700));
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    place(&mut t, "1/500");
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));

    server.stall(Stall::Sequence);
    t.send(Key::Char('G')).unwrap();
    t.wait_until(|s| hint_line(s).contains("syncing 0/200"))
        .unwrap();

    server.fail(Outage::Offline);
    server.release();
    t.wait_until(|s| hint_line(s).contains("offline")).unwrap();

    server.recover();
    place(&mut t, "500/700");
    quit(t);
    assert_eq!(cached(&home, "SELECT count(*) FROM message"), 700);
}
