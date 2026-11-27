//! A Folder far longer than the window of cards the Message List reads
//! around the selection: docs/adr/0022-the-message-list-holds-thread-keys-and-reads-cards-for-the-window.md.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use support::{
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage},
    home::Home,
    quit, wait_for,
};
use termlens::{Key, Screen, Terminal};

/// At 20 rows a list shows 6 cards and reads 3 screens either side.
const MESSAGES: u32 = 150;

fn postal(home: &Home) -> Terminal {
    home.terminal()
        .size(100, 20)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn idles(server: &FakeImap) -> usize {
    server.commands().iter().filter(|c| *c == "IDLE").count()
}

fn subject(n: u32) -> String {
    format!("Message {n:03}")
}

/// Message `n` from Sender `n`, a day after the one before; UID
/// `FIRST_UID + n`.
fn message(n: u32) -> FakeMessage {
    let date = chrono::DateTime::UNIX_EPOCH + chrono::Duration::days(19_000 + i64::from(n));
    let sender: &'static str = Box::leak(format!("Sender {n:03}").into_boxed_str());
    FakeMessage::new(sender, &subject(n), &date.to_rfc3339())
        .body(FakeBody::plain(&format!("Body of {}", subject(n))))
}

fn messages() -> Vec<FakeMessage> {
    (0..MESSAGES).map(message).collect()
}

/// Started, the sync idling and the newest Message's Preview in.
fn ready(server: &FakeImap, home: &Home) -> Terminal {
    let mut t = postal(home);
    t.wait_until(|s| s.contains(&format!("Body of {}", subject(MESSAGES - 1))))
        .unwrap();
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

/// Whether the card of `subject` shows the Unread marker on its sender line.
fn unread(screen: &Screen, subject: &str) -> Option<bool> {
    let row = (0..screen.rows()).find(|&row| screen.row_text(row).contains(subject))?;
    Some(screen.cell(row.checked_sub(1)?, 2)?.contents() == "●")
}

fn shows(screen: &Screen, selected: u32, position: &str) -> bool {
    selected_subject(screen) == Some(subject(selected))
        && screen.row_text(0).contains(position)
        && screen.contains(&format!("Body of {}", subject(selected)))
}

// Cards past the first window are read as the selection gets there, and
// those of the top again on the way back.
#[test]
fn a_folder_longer_than_the_window_scrolls_to_its_end_and_back() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('G')).unwrap();
    t.wait_until(|s| shows(s, 0, " 150/150 ")).unwrap();
    for _ in 0..40 {
        t.send(Key::Char('k')).unwrap();
    }

    t.wait_until(|s| shows(s, 40, " 110/150 ")).unwrap();
    t.send(Key::Char('g')).unwrap();
    t.wait_until(|s| shows(s, MESSAGES - 1, " 1/150 ")).unwrap();
    quit(t);
}

// Each kind of change rereads the rows and the window, and the selection
// far down the Folder stays on its Thread through all of them.
#[test]
fn the_selection_past_the_first_window_stays_on_its_thread_as_the_folder_changes() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    t.send(Key::Char('G')).unwrap();
    for _ in 0..5 {
        t.send(Key::Char('k')).unwrap();
    }

    t.wait_until(|s| shows(s, 5, " 145/150 ")).unwrap();

    server.deliver(message(MESSAGES).unread());
    t.wait_until(|s| shows(s, 5, " 146/151 ")).unwrap();

    server.expunge(FIRST_UID);
    t.wait_until(|s| shows(s, 5, " 146/150 ")).unwrap();

    server.set_seen(FIRST_UID + 6, false);
    let screen = t
        .snapshot_after(|s| unread(s, &subject(6)) == Some(true))
        .unwrap();
    quit(t);
    assert!(shows(&screen, 5, " 146/150 "), "{screen}");
}

// The Thread's reply in Sent comes with the window, so Reading a Thread
// far from the top shows it like one at the top.
#[test]
fn reading_a_thread_past_the_first_window_shows_its_message_from_another_folder() {
    let mut inbox = vec![
        FakeMessage::new("Bob", "Re: Trip plans", "2020-01-02T09:00:00Z")
            .body(FakeBody::plain("Count me in"))
            .in_thread(7),
    ];
    inbox.extend(messages());
    let server = FakeImap::start(inbox);
    server.add_folder(
        "[Gmail]/Sent Mail",
        &["\\Sent"],
        vec![
            FakeMessage::new("Me", "Trip plans", "2020-01-01T09:00:00Z")
                .body(FakeBody::plain("The coast?"))
                .in_thread(7)
                .sent_by_me(),
        ],
    );
    server.add_all_mail("[Gmail]/All Mail");
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('G')).unwrap();
    t.wait_until(|s| s.row_text(0).contains(" 151/151 ") && s.contains("Count me in"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.row_text(0).contains(" 2/2 ") && s.contains("The coast?"))
        .unwrap();
    t.send(Key::Char('q')).unwrap();
    let list = t.snapshot_after(|s| s.contains("Bob (2)")).unwrap();
    quit(t);
    assert_eq!(
        selected_subject(&list).as_deref(),
        Some("Re: Trip plans"),
        "{list}"
    );
}
