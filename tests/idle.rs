#[allow(dead_code, reason = "each test binary uses part of the harness")]
mod support;

use std::time::{Duration, Instant};

use support::{
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

const ITEMS: &str =
    "(UID FLAGS INTERNALDATE ENVELOPE X-GM-MSGID BODYSTRUCTURE BODY.PEEK[HEADER.FIELDS (FROM)])";

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
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn wait_for(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn idles(server: &FakeImap) -> usize {
    server.commands().iter().filter(|c| *c == "IDLE").count()
}

/// Starts Postal and waits until the sync is done and idling.
fn idling(server: &FakeImap, home: &Home) -> Terminal {
    let t = postal(home);
    wait_for(|| idles(server) > 0);
    t
}

/// The subject line of the card drawn in reverse video, the selection.
fn selected_subject(screen: &Screen) -> Option<String> {
    let first =
        (0..screen.rows()).find(|&row| screen.cell(row, 3).is_some_and(|c| c.style().reverse))?;
    let subject = screen.row_text(first + 1);
    let subject = subject.split('│').nth(1)?;
    Some(subject.trim().to_owned())
}

/// Whether the card naming `sender` shows the Unread marker; `None` when
/// no card on screen names it.
fn card(screen: &Screen, sender: &str) -> Option<bool> {
    let row = (0..screen.rows()).find(|&row| screen.row_text(row).contains(sender))?;
    Some(screen.cell(row, 1)?.contents() == "●")
}

fn message(sender: &'static str, subject: &str, date: &str) -> FakeMessage {
    FakeMessage::new(sender, subject, date).body(FakeBody::plain(&format!("Body of {subject}")))
}

/// UIDs 101 to 104, newest first Carol, Alice, Dave, Bob; Carol Unread.
fn four_messages() -> Vec<FakeMessage> {
    vec![
        message("Alice", "Second newest", "2024-03-02T09:00:00Z"),
        message("Bob", "Oldest", "2023-12-31T23:59:00Z"),
        message("Carol", "Newest", "2024-05-10T08:30:00Z").unread(),
        message("Dave", "Third", "2024-01-15T12:00:00Z"),
    ]
}

#[test]
fn new_mail_appears_above_and_the_selection_stays_on_its_message() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    let mut t = idling(&server, &home);
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| {
        selected_subject(s).as_deref() == Some("Second newest")
            && s.contains("Body of Second newest")
    })
    .unwrap();

    let uid = server.deliver(message("Erin", "Brand new", "2024-06-01T00:00:00Z").unread());
    let screen = t
        .snapshot_after(|s| s.contains("Body of Brand new") && s.contains(" Inbox  2 unread "))
        .unwrap();
    quit(t);

    assert_eq!(card(&screen, "Erin"), Some(true), "{screen}");
    assert_eq!(
        selected_subject(&screen).as_deref(),
        Some("Second newest"),
        "{screen}"
    );
    let commands = server.commands();
    let fetch = format!("UID FETCH {uid}:* {ITEMS}");
    let after_done = commands.iter().skip_while(|c| *c != "DONE");
    assert!(after_done.clone().any(|c| *c == fetch), "{commands:#?}");
    // Phase 2 cuts the new row's Snippet, then the connection idles again.
    let snippet = after_done.clone().position(|c| c.contains("BODY.PEEK[1]<"));
    let idle = after_done.clone().position(|c| c == "IDLE");
    assert!(snippet.is_some() && snippet < idle, "{commands:#?}");
}

#[test]
fn an_expunged_selection_moves_to_the_next_older_row_and_the_preview_follows() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    let mut t = idling(&server, &home);
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains("Body of Second newest"))
        .unwrap();

    server.expunge(FIRST_UID);
    let screen = t
        .snapshot_after(|s| !s.contains("Alice") && s.contains("Body of Third"))
        .unwrap();
    quit(t);
    assert_eq!(
        selected_subject(&screen).as_deref(),
        Some("Third"),
        "{screen}"
    );
}

// Each EXPUNGE renumbers the Messages after it: read against the old
// numbers, the second one would take out Bob instead of Carol.
#[test]
fn expunges_in_one_burst_each_renumber_the_rest() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    let mut t = idling(&server, &home);
    t.wait_until(|s| s.contains("Body of Newest")).unwrap();

    server.expunge(FIRST_UID);
    server.expunge(FIRST_UID + 2);
    let screen = t
        .snapshot_after(|s| !s.contains("Alice") && !s.contains("Carol"))
        .unwrap();
    quit(t);
    for kept in ["Bob", "Dave"] {
        assert!(screen.contains(kept), "{kept}: {screen}");
    }

    assert!(screen.contains(" Inbox  0 unread "), "{screen}");
    assert_eq!(
        selected_subject(&screen).as_deref(),
        Some("Third"),
        "{screen}"
    );
}

#[test]
fn a_flag_change_updates_unread_and_the_count() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    let mut t = idling(&server, &home);

    server.set_seen(FIRST_UID + 2, true);
    server.set_seen(FIRST_UID + 1, false);
    let screen = t
        .snapshot_after(|s| card(s, "Carol") == Some(false) && card(s, "Bob") == Some(true))
        .unwrap();
    quit(t);
    assert!(screen.contains(" Inbox  1 unread "), "{screen}");
    assert_eq!(card(&screen, "Alice"), Some(false), "{screen}");
}

// Messages older than the Cache still count in the sequence numbers: an
// expunge among them shifts every cached row's number down by one.
#[test]
fn an_expunge_older_than_the_cache_renumbers_the_cached_rows() {
    let messages = (0..503)
        .map(|i| {
            let date = chrono::DateTime::UNIX_EPOCH + chrono::Duration::hours(i);
            FakeMessage::new("Sender", &format!("Message {}", i + 1), &date.to_rfc3339())
        })
        .collect();
    let server = FakeImap::start(messages);
    let home = Home::for_server(&server);
    let mut t = idling(&server, &home);

    // UID 101 is older than the Cache's 104 to 603.
    server.expunge(FIRST_UID);
    wait_for(|| idles(&server) > 1);
    server.set_seen(FIRST_UID + 502, false);
    let screen = t
        .snapshot_after(|s| s.contains(" Inbox  1 unread "))
        .unwrap();
    quit(t);

    let marked = |subject: &str| {
        let row = (0..screen.rows()).find(|&row| screen.row_text(row).contains(subject));
        row.and_then(|row| screen.cell(row - 1, 1))
            .map(|cell| cell.contents() == "●")
    };

    assert_eq!(marked("Message 503 "), Some(true), "{screen}");
    assert_eq!(marked("Message 502 "), Some(false), "{screen}");
}

// The IMAP library keeps at most 100 announcements and drops the rest
// unseen: a bulk delete past that must still remove exactly its rows.
#[test]
fn a_burst_past_what_the_library_keeps_still_removes_every_row() {
    let messages = (0..300)
        .map(|i| {
            let date = chrono::DateTime::UNIX_EPOCH + chrono::Duration::hours(i);
            FakeMessage::new("Sender", &format!("Message {}", i + 1), &date.to_rfc3339())
        })
        .collect();
    let server = FakeImap::start(messages);
    let home = Home::for_server(&server);
    let mut t = idling(&server, &home);
    t.wait_until(|s| s.contains("Message 300 ")).unwrap();

    // Messages 151 to 300, the newest half.
    server.expunge_range(FIRST_UID + 150..=FIRST_UID + 299);
    let screen = t
        .snapshot_after(|s| s.contains("Message 150 ") && !s.contains("Message 300 "))
        .unwrap();
    quit(t);
    assert!(!screen.contains("Message 151 "), "{screen}");
}
