//! A desktop notification for new Unread mail in an Inbox, through the
//! fake `osascript` and `notify-send` ahead on `PATH`, or the terminal's
//! own escape.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{fs, time::Duration};

use support::{
    fake_body::FakeBody,
    fake_imap::{EMAIL, FakeImap, FakeMessage},
    home::Home,
    notifier::Notifier,
    picker::picked,
    quit, wait_for,
};
use termlens::{Key, Terminal, TerminalBuilder};

/// Long enough for a notification the screen already shows to reach the file.
const SETTLE: Duration = Duration::from_millis(300);

fn message(sender: &'static str, subject: &str, date: &str) -> FakeMessage {
    FakeMessage::new(sender, subject, date).body(FakeBody::plain(&format!("Body of {subject}")))
}

fn inbox() -> FakeImap {
    FakeImap::start(vec![message(
        "Alice",
        "Weekly notes",
        "2024-03-02T09:00:00Z",
    )])
}

/// `config.toml` for `server`, with `notify` set when given.
fn home(server: &FakeImap, notify: Option<&str>) -> Home {
    let home = Home::for_server(server);
    if let Some(notify) = notify {
        let config = fs::read_to_string(home.config_path()).unwrap();
        fs::write(
            home.config_path(),
            format!("notify = \"{notify}\"\n{config}"),
        )
        .unwrap();
    }

    home
}

fn builder(home: &Home, notifier: &Notifier) -> TerminalBuilder {
    home.terminal().size(120, 30).env("PATH", notifier.path())
}

/// Postal once the Inbox shows and the sync connection idles on it.
fn idling(server: &FakeImap, t: TerminalBuilder) -> Terminal {
    let mut t = t.spawn(env!("CARGO_BIN_EXE_postal")).unwrap();
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    t
}

// By default the user is told only while looking elsewhere: mail that
// comes while the terminal has focus is in plain sight already.
#[test]
fn new_mail_notifies_once_only_while_the_terminal_has_no_focus() {
    let server = inbox();
    let home = home(&server, None);
    let notifier = Notifier::new();
    let mut t = idling(&server, builder(&home, &notifier));
    t.focus_in().unwrap();

    server.deliver(message("Bob", "Seen arriving", "2024-06-01T00:00:00Z").unread());
    t.wait_until(|s| s.contains("Seen arriving")).unwrap();
    std::thread::sleep(SETTLE);
    assert_eq!(notifier.notified(), Vec::<String>::new());

    t.focus_out().unwrap();
    server.deliver(message("Erin", "Brand new", "2024-06-02T00:00:00Z").unread());
    t.wait_until(|s| s.contains("Brand new")).unwrap();
    assert_eq!(notifier.wait_for(1), ["Erin | Brand new"]);
    std::thread::sleep(SETTLE);
    assert_eq!(notifier.notified(), ["Erin | Brand new"]);

    t.focus_in().unwrap();
    server.deliver(message("Finn", "Back again", "2024-06-03T00:00:00Z").unread());
    t.wait_until(|s| s.contains("Back again")).unwrap();
    std::thread::sleep(SETTLE);
    quit(t);
    assert_eq!(notifier.notified(), ["Erin | Brand new"]);
}

// Mail read elsewhere already, or sent by the Account to itself, is no news.
#[test]
fn mail_seen_already_or_from_the_account_notifies_of_nothing() {
    let server = inbox();
    let home = home(&server, Some("always"));
    let notifier = Notifier::new();
    let t = idling(&server, builder(&home, &notifier));

    server.deliver(message("Bob", "Read on the phone", "2024-06-01T00:00:00Z"));
    let mut own = message("Me", "Note to self", "2024-06-02T00:00:00Z").unread();
    own.from_address = EMAIL;
    server.deliver(own);
    server.deliver(message("Erin", "Brand new", "2024-06-03T00:00:00Z").unread());
    assert_eq!(notifier.wait_for(1), ["Erin | Brand new"]);
    std::thread::sleep(SETTLE);
    quit(t);
    assert_eq!(notifier.notified(), ["Erin | Brand new"]);
}

// IDLE watches the shown Folder alone; `STATUS` finds the Inbox's new mail,
// and what one look finds comes as one notification.
#[test]
fn new_mail_in_the_inbox_notifies_while_another_folder_shows() {
    let server = inbox();
    server.add_folder(
        "Work",
        &[],
        vec![message("Carol", "Quarterly plan", "2024-01-15T12:00:00Z")],
    );
    let home = home(&server, Some("always"));
    let notifier = Notifier::new();
    let mut t = idling(
        &server,
        builder(&home, &notifier).env("POSTAL_INBOX_POLL", "1"),
    );
    t.send(Key::Char('f')).unwrap();
    t.send_str("Work").unwrap();
    // The Cache lists the Inbox alone until the server's list comes.
    t.snapshot_after(|s| matches!(&picked(s)[..], [work] if work.starts_with("Work")))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("Quarterly plan") && !s.contains("Weekly notes"))
        .unwrap();

    server.clear_commands();
    server.deliver(message("Erin", "Brand new", "2024-06-01T00:00:00Z").unread());
    server.deliver(message("Finn", "Also new", "2024-06-02T00:00:00Z").unread());
    server.deliver(message("Erin", "And another", "2024-06-03T00:00:00Z").unread());
    assert_eq!(notifier.wait_for(1), ["3 new messages | Erin, Finn"]);
    // Back to idling on the shown Folder, after a resync of it.
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    quit(t);

    let commands = server.commands();
    assert!(
        commands.contains(&"STATUS \"INBOX\" (UIDNEXT)".to_owned()),
        "{commands:#?}"
    );
    assert_eq!(notifier.notified().len(), 1);
}

// Mail that came while Postal was closed is found by the first sync, and
// older mail by a Backfill: neither is news.
#[test]
fn the_first_sync_and_a_backfill_notify_of_nothing() {
    let older = (1..=600).map(|n| {
        let date = chrono::DateTime::UNIX_EPOCH + chrono::Duration::hours(n);
        message("Sender", &format!("Message {n}"), &date.to_rfc3339()).unread()
    });
    let server = FakeImap::start(older.collect());
    let home = home(&server, Some("always"));
    let notifier = Notifier::new();
    let run = || {
        builder(&home, &notifier)
            .spawn(env!("CARGO_BIN_EXE_postal"))
            .unwrap()
    };

    let mut t = run();
    t.wait_until(|s| s.contains("Message 600")).unwrap();
    quit(t);

    server.deliver(message("Erin", "While closed", "2024-06-01T00:00:00Z").unread());
    let mut t = run();
    t.wait_until(|s| s.contains("While closed")).unwrap();
    wait_for(|| server.commands().iter().filter(|c| *c == "IDLE").count() > 1);
    t.send(Key::Char('G')).unwrap();
    t.wait_until(|s| s.row_text(0).contains(" 501/601 "))
        .unwrap();
    t.send(Key::Char('G')).unwrap();
    t.wait_until(|s| s.contains("Message 1\u{20}")).unwrap();
    std::thread::sleep(SETTLE);
    quit(t);
    assert_eq!(notifier.notified(), Vec::<String>::new());
}

// A terminal known to show `OSC 9` as a desktop notification gets the
// escape, and the OS is left alone.
#[test]
fn a_terminal_that_shows_notifications_gets_its_own_escape() {
    let server = inbox();
    let home = home(&server, Some("always"));
    let notifier = Notifier::new();
    let mut t = idling(
        &server,
        builder(&home, &notifier).env("TERM_PROGRAM", "WezTerm"),
    );

    server.deliver(message("Erin", "Brand new", "2024-06-01T00:00:00Z").unread());
    t.wait_until(|s| s.unsupported().contains("^[]9;Erin: Brand new"))
        .unwrap();
    quit(t);
    assert_eq!(notifier.notified(), Vec::<String>::new());
}

// Settings changes `notify` in `config.toml`, and the running TUI follows.
#[test]
fn notify_set_in_settings_takes_effect_at_once() {
    let server = inbox();
    let home = home(&server, None);
    let notifier = Notifier::new();
    let mut t = idling(&server, builder(&home, &notifier));
    t.send(Key::Char(',')).unwrap();
    t.wait_until(|s| s.contains("Notify")).unwrap();
    // Past the Accounts and Icons tabs.
    t.send(Key::Tab).unwrap();
    t.send(Key::Tab).unwrap();
    t.wait_until(|s| s.contains("unfocused ✓")).unwrap();
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("always ✓")).unwrap();
    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(config.contains("notify = \"always\""), "{config}");
    t.send(Key::Esc).unwrap();
    t.wait_until(|s| !s.contains("Notify")).unwrap();

    server.deliver(message("Erin", "Brand new", "2024-06-01T00:00:00Z").unread());
    assert_eq!(notifier.wait_for(1), ["Erin | Brand new"]);
    quit(t);
}
