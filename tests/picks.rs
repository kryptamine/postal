//! Picks with `J` and `K`, and Delete, Archive and `U` on every picked
//! Thread at once, through the fake server.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::time::{Duration, Instant};

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage, INBOX},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

const ARCHIVE: &str = "Archive";
const TRASH: &str = "[Gmail]/Trash";
const READ: &str = "+FLAGS.SILENT (\\Seen)";
const UNREAD: &str = "-FLAGS.SILENT (\\Seen)";

/// The Message List's columns beside the Preview at 120 columns.
const LIST: usize = 42;

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

#[track_caller]
fn wait_for(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "timed out at {}",
            std::panic::Location::caller()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn message(sender: &'static str, subject: &str, date: &str) -> FakeMessage {
    FakeMessage::new(sender, subject, date).body(FakeBody::plain(&format!("Body of {subject}")))
}

/// An Inbox of three Threads, newest first: Carol's, Bob's and Alice's,
/// beside an empty Archive and Trash.
fn server(messages: [FakeMessage; 3]) -> FakeImap {
    let server = FakeImap::start(messages.into());
    server.without_gmail();
    server.add_folder(ARCHIVE, &["\\Archive"], vec![]);
    server.add_folder(TRASH, &["\\Trash"], vec![]);
    server
}

fn three() -> [FakeMessage; 3] {
    [
        message("Alice", "Oldest", "2024-03-01T09:00:00Z"),
        message("Bob", "Middle", "2024-03-02T09:00:00Z"),
        message("Carol", "Newest", "2024-03-03T09:00:00Z"),
    ]
}

/// Started on the newest Message's Preview, with the sync idling.
fn ready(server: &FakeImap, home: &Home, newest: &str) -> Terminal {
    let mut t = postal(home);
    t.wait_until(|s| s.contains(&format!("Body of {newest}")))
        .unwrap();
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    t
}

fn list(screen: &Screen) -> String {
    (0..screen.rows())
        .map(|row| screen.row_text(row).chars().take(LIST).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn hint_line(screen: &Screen) -> String {
    screen.row_text(screen.rows() - 1)
}

/// The senders whose card shows the Pick's mark.
fn picked(screen: &Screen) -> Vec<String> {
    let marked = |row| screen.cell(row, 1).is_some_and(|c| c.contents() == "▌");
    let senders = ["Alice", "Bob", "Carol"];
    (0..screen.rows())
        .filter(|&row| marked(row))
        .filter_map(|row| {
            let text: String = screen.row_text(row).chars().take(LIST).collect();
            senders.into_iter().find(|s| text.contains(s))
        })
        .map(String::from)
        .collect()
}

fn stores(server: &FakeImap) -> Vec<String> {
    server
        .commands()
        .into_iter()
        .filter(|c| c.starts_with("UID STORE"))
        .collect()
}

#[test]
fn j_picks_down_and_an_archive_takes_them_all_back_in_one_undo() {
    let server = server(three());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");

    t.send(Key::Char('J')).unwrap();
    let screen = t
        .snapshot_after(|s| hint_line(s).contains("2 picked"))
        .unwrap();
    assert_eq!(picked(&screen), ["Carol", "Bob"], "{screen}");

    t.send(Key::Char('e')).unwrap();
    let screen = t
        .snapshot_after(|s| !list(s).contains("Newest") && !list(s).contains("Middle"))
        .unwrap();
    assert!(
        hint_line(&screen).contains("2 archived · u to undo"),
        "{screen}"
    );
    assert!(picked(&screen).is_empty(), "{screen}");
    wait_for(|| server.subjects(ARCHIVE).len() == 2);
    assert_eq!(server.subjects(INBOX), ["Oldest"]);

    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| list(s).contains("Newest") && list(s).contains("Middle"))
        .unwrap();
    wait_for(|| server.subjects(ARCHIVE).is_empty() && server.subjects(INBOX).len() == 3);
    quit(t);
}

#[test]
fn k_picks_up_and_a_delete_takes_only_those() {
    let server = server(three());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");

    t.send_str("GK").unwrap();
    let screen = t
        .snapshot_after(|s| hint_line(s).contains("2 picked"))
        .unwrap();
    assert_eq!(picked(&screen), ["Bob", "Alice"], "{screen}");

    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| hint_line(s).contains("2 deleted · u to undo"))
        .unwrap();
    wait_for(|| server.subjects(TRASH).len() == 2);
    quit(t);
    assert_eq!(server.subjects(INBOX), ["Newest"]);
}

#[test]
fn esc_or_a_plain_move_drops_the_picks() {
    let server = server(three());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");

    t.send_str("JJ").unwrap();
    t.wait_until(|s| hint_line(s).contains("3 picked")).unwrap();
    t.send(Key::Esc).unwrap();
    let screen = t
        .snapshot_after(|s| !hint_line(s).contains("picked"))
        .unwrap();
    assert!(picked(&screen).is_empty(), "{screen}");

    t.send(Key::Char('K')).unwrap();
    t.wait_until(|s| hint_line(s).contains("2 picked")).unwrap();
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| !hint_line(s).contains("picked")).unwrap();

    // With no Picks, an action takes the selected Thread alone.
    t.send(Key::Char('e')).unwrap();
    t.wait_until(|s| hint_line(s).contains("archived") && !list(s).contains("Oldest"))
        .unwrap();
    wait_for(|| server.subjects(ARCHIVE) == ["Oldest"]);
    quit(t);
}

// Threads past the window around the selection have no cards read; the
// action reads their Messages itself.
#[test]
fn picks_past_the_window_are_archived_whole() {
    let messages = (0..60).map(|i| {
        let date = chrono::DateTime::UNIX_EPOCH + chrono::Duration::hours(i);
        message("Sender", &format!("Message {}", i + 1), &date.to_rfc3339())
    });
    let server = FakeImap::start(messages.collect());
    server.without_gmail();
    server.add_folder(ARCHIVE, &["\\Archive"], vec![]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Message 60");

    t.send_str(&"J".repeat(59)).unwrap();
    t.wait_until(|s| hint_line(s).contains("60 picked"))
        .unwrap();
    t.send(Key::Char('e')).unwrap();
    t.wait_until(|s| hint_line(s).contains("60 archived"))
        .unwrap();
    wait_for(|| server.subjects(ARCHIVE).len() == 60);
    quit(t);
    assert_eq!(server.subjects(INBOX), Vec::<String>::new());
}

#[test]
fn u_on_the_picks_marks_them_read_while_any_is_unread_else_unread() {
    let [oldest, middle, newest] = three();
    let server = server([oldest.unread(), middle, newest]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");

    // Back on Carol's read Thread at once, so no Dwell marks Alice's.
    t.send_str("GKK").unwrap();
    t.wait_until(|s| hint_line(s).contains("3 picked")).unwrap();
    t.send(Key::Char('U')).unwrap();
    t.wait_until(|s| hint_line(s).contains("3 marked Read"))
        .unwrap();
    wait_for(|| stores(&server).len() == 1);
    assert!(stores(&server)[0].ends_with(READ), "{:?}", stores(&server));

    t.send_str("JJ").unwrap();
    t.wait_until(|s| hint_line(s).contains("3 picked")).unwrap();
    t.send(Key::Char('U')).unwrap();
    t.wait_until(|s| hint_line(s).contains("3 marked Unread"))
        .unwrap();
    wait_for(|| stores(&server).len() == 2);
    quit(t);
    let stores = stores(&server);
    assert!(stores[1].ends_with(UNREAD), "{stores:?}");
}
