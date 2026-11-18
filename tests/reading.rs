//! `Enter` reads the selected Message full screen, with keys of its own,
//! until `q` or `Esc` goes back to the list.

#[allow(dead_code, reason = "each test binary uses part of the harness")]
mod support;

use std::time::Duration;

use support::{
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

fn postal(home: &Home) -> Terminal {
    Terminal::builder()
        .size(100, 16)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

/// Forty numbered lines, more than one screen holds.
fn long_body() -> FakeBody {
    let lines: Vec<String> = (1..=40).map(|n| format!("Line {n:02}")).collect();
    FakeBody::plain(&lines.join("\r\n"))
}

fn two_messages() -> Vec<FakeMessage> {
    vec![
        FakeMessage::new("Ada", "Older", "2024-11-17T09:00:00Z")
            .body(FakeBody::plain("Older body")),
        FakeMessage::new("Bob", "Newer", "2024-11-18T09:00:00Z").body(long_body()),
    ]
}

fn listed(screen: &Screen) -> bool {
    screen.contains(" Inbox ") && screen.contains("Line 01")
}

fn reading(screen: &Screen) -> bool {
    !screen.contains(" Inbox ") && screen.row_text(screen.rows() - 1).contains("q back")
}

// The list gives way to the Message, and j/k move through it, not through
// the list: the next Message never shows.
#[test]
fn enter_reads_the_message_full_screen_and_q_goes_back() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(listed).unwrap();

    t.send(Key::Enter).unwrap();
    let screen = t.snapshot_after(reading).unwrap();
    assert!(screen.contains("From    Bob"), "{screen}");

    t.send(Key::Char('j')).unwrap();
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| !s.contains("From    Bob") && s.contains("Line 01"))
        .unwrap();
    t.send(Key::Char('G')).unwrap();
    let screen = t.snapshot_after(|s| s.contains("Line 40")).unwrap();
    assert!(!screen.contains("Older body"), "{screen}");
    t.send(Key::Char('g')).unwrap();
    t.wait_until(|s| s.contains("From    Bob")).unwrap();

    // `q` leaves the Message, not Postal.
    t.send(Key::Char('q')).unwrap();
    t.wait_until(listed).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(reading).unwrap();
    t.send(Key::Esc).unwrap();
    t.wait_until(listed).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

// The full-screen Message belongs to the Message it was opened on: once
// that one is gone, the list comes back on the one selected after it.
#[test]
fn reading_ends_when_the_message_is_expunged() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(listed).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(reading).unwrap();

    server.expunge(FIRST_UID + 1);
    t.wait_until(|s| s.contains(" Inbox ") && s.contains("Older body"))
        .unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[test]
fn the_help_popup_lists_the_reading_keys() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = Terminal::builder()
        .size(80, 24)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    t.wait_until(|s| s.contains(" unread ")).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(reading).unwrap();
    t.send(Key::Char('?')).unwrap();
    let screen = t.snapshot_after(|s| s.contains(" Help ")).unwrap();
    insta::assert_snapshot!(screen);
    t.send(Key::Esc).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}
