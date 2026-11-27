//! Reading a Gmail Thread finds its Messages in other Folders through All
//! Mail, by `X-GM-THRID`, and adds them to the stack and to the row's count.

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
    fake_imap::{FakeImap, FakeMessage, Outage, Stall},
    home::Home,
    quit, wait_for,
};
use termlens::{Key, Screen, Terminal};

fn postal(home: &Home) -> Terminal {
    home.terminal()
        .size(100, 20)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

/// "Count me in", then more lines than one screen holds.
fn long_body() -> FakeBody {
    let lines: Vec<String> = (1..=40).map(|n| format!("Line {n:02}")).collect();
    FakeBody::plain(&format!("Count me in\r\n{}", lines.join("\r\n")))
}

fn message(sender: &'static str, (subject, body): (&str, &str), date: &str) -> FakeMessage {
    FakeMessage::new(sender, subject, date).body(FakeBody::plain(body))
}

/// Thread 0: the Account asks in Sent, Bob answers in the Inbox, and Eve's
/// reply lies in Trash. Carol's lone Message sits in the Inbox beside it.
fn gmail() -> FakeImap {
    let server = FakeImap::start(vec![
        message("Carol", ("Newsletter", "This week"), "2024-03-01T08:00:00Z"),
        FakeMessage::new("Bob", "Re: Trip plans", "2024-03-03T09:00:00Z")
            .body(long_body())
            .in_thread(0),
    ]);
    server.add_folder(
        "[Gmail]/Sent Mail",
        &["\\Sent"],
        vec![
            message("Me", ("Trip plans", "The coast?"), "2024-03-02T09:00:00Z")
                .in_thread(0)
                .sent_by_me(),
        ],
    );
    server.add_folder(
        "[Gmail]/Trash",
        &["\\Trash"],
        vec![
            message(
                "Eve",
                ("Re: Trip plans", "Binned reply"),
                "2024-03-02T12:00:00Z",
            )
            .in_thread(0),
        ],
    );
    server.add_all_mail("[Gmail]/All Mail");
    server
}

fn idles(server: &FakeImap) -> usize {
    server.commands().iter().filter(|c| *c == "IDLE").count()
}

/// Started, with the Inbox shown and the sync idling.
fn ready(server: &FakeImap, home: &Home) -> Terminal {
    let mut t = postal(home);
    t.wait_until(|s| s.contains("Count me in")).unwrap();
    wait_for(|| idles(server) > 0);
    t
}

fn reading(screen: &Screen) -> bool {
    !screen.contains(" Inbox ") && screen.contains("From    Bob")
}

fn row_of(screen: &Screen, text: &str) -> Option<u16> {
    (0..screen.rows()).find(|&row| screen.row_text(row).contains(text))
}

fn searched_thread(server: &FakeImap) -> bool {
    server
        .commands()
        .iter()
        .any(|c| c.starts_with("UID SEARCH X-GM-THRID"))
}

// The reply sent from the Account lives in Sent alone, yet belongs to the
// conversation: it joins above Bob without moving him, and the row counts it.
#[test]
fn a_reply_in_sent_joins_the_stack_and_the_count() {
    let server = gmail();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    let screen = t.snapshot_after(|_| true).unwrap();
    assert!(!screen.contains("Bob (2)"), "{screen}");

    // Held until Reading shows Bob alone.
    server.stall(Stall::Body);
    t.send(Key::Enter).unwrap();
    let before = t.snapshot_after(reading).unwrap();
    assert!(before.row_text(0).contains(" 1/1 "), "{before}");
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Char('j')).unwrap();
    let scrolled = t
        .snapshot_after(|s| !s.contains("Line 01") && s.contains("Line 05"))
        .unwrap();
    let line = row_of(&scrolled, "Line 05");
    server.release();

    let grown = t
        .snapshot_after(|s| s.row_text(0).contains(" 2/2 "))
        .unwrap();
    assert_eq!(row_of(&grown, "Line 05"), line, "{grown}");

    t.send(Key::Char('g')).unwrap();
    let top = t.snapshot_after(|s| s.contains("The coast?")).unwrap();
    let me = row_of(&top, "The coast?").unwrap();
    assert!(top.row_text(me).contains("me"), "{top}");
    assert!(!top.contains("Binned reply"), "{top}");

    t.send(Key::Char('q')).unwrap();
    let list = t.snapshot_after(|s| s.contains(" Inbox ")).unwrap();
    assert!(list.contains("Bob (2)"), "{list}");
    assert!(!list.contains("Binned reply"), "{list}");
    quit(t);
}

// The sync connection is the one that idles: the search runs between two
// IDLEs, and the next one watches the Inbox again, so new mail still shows.
#[test]
fn idle_resumes_on_the_shown_folder_after_the_thread_is_fetched() {
    let server = gmail();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    server.clear_commands();

    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.row_text(0).contains(" 2/2 ")).unwrap();
    wait_for(|| idles(&server) > 0);
    let commands = server.commands();
    let at = |wanted: &dyn Fn(&str) -> bool| commands.iter().position(|c| wanted(c));
    let all_mail = at(&|c| c == "EXAMINE \"[Gmail]/All Mail\"").unwrap();
    let search = at(&|c| c.starts_with("UID SEARCH X-GM-THRID")).unwrap();
    let inbox = commands
        .iter()
        .rposition(|c| c == "SELECT \"INBOX\"")
        .unwrap();
    let idle = commands.iter().rposition(|c| c == "IDLE").unwrap();
    assert!(
        all_mail < search && search < inbox && inbox < idle,
        "{commands:#?}"
    );

    server.deliver(message("Dan", ("Lunch", "Noon?"), "2024-03-04T09:00:00Z"));
    t.send(Key::Char('q')).unwrap();
    t.wait_until(|s| s.contains(" Inbox ") && s.contains("Noon?"))
        .unwrap();
    quit(t);
}

// Without a connection, Reading shows what the Cache holds of the shown
// Folder and does not wait on the server.
#[test]
fn offline_reading_shows_the_folders_messages_only() {
    let server = gmail();
    let home = Home::for_server(&server);
    quit(ready(&server, &home));

    server.fail(Outage::Offline);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Count me in") && s.contains("offline"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(reading).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let screen = t.snapshot_after(|_| true).unwrap();
    assert!(screen.row_text(0).contains(" 1/1 "), "{screen}");
    assert!(!screen.contains("The coast?"), "{screen}");
    quit(t);
}

// Elsewhere each Message is a Thread of its own: there is nothing to look for.
#[test]
fn a_server_without_gmail_is_not_searched_for_the_thread() {
    let server = gmail();
    server.without_gmail();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Enter).unwrap();
    t.wait_until(reading).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert!(!searched_thread(&server), "{:#?}", server.commands());
    quit(t);
}
