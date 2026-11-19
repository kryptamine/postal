//! The hint line along the bottom, the `?` popup over the panes, and the
//! Toasts in the hint line's right slot.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{fmt::Write, time::Duration};

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage, Stall},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

fn postal(home: &Home, (cols, rows): (u16, u16)) -> Terminal {
    Terminal::builder()
        .size(cols, rows)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn two_messages() -> Vec<FakeMessage> {
    vec![
        FakeMessage::new("Ada", "Older", "2024-11-17T09:00:00Z")
            .body(FakeBody::plain("Older body")),
        FakeMessage::new("Bob", "Newer", "2024-11-18T09:00:00Z")
            .body(FakeBody::plain("Newer body")),
    ]
}

fn loaded(screen: &Screen) -> bool {
    screen.contains(" unread ")
}

fn hint_line(screen: &Screen) -> String {
    screen.row_text(screen.rows() - 1)
}

fn popup_open(screen: &Screen) -> bool {
    screen.contains(" Help ") && screen.contains("Navigate")
}

fn quit(mut t: Terminal) {
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[test]
fn the_hint_line_drops_entries_from_the_right_and_keeps_help() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut lines = String::new();
    for cols in [100, 62, 61, 40, 20, 9] {
        let mut t = postal(&home, (cols, 12));
        let screen = t.snapshot_after(|s| hint_line(s).contains("help")).unwrap();
        writeln!(lines, "{cols:>3} |{}|", hint_line(&screen)).unwrap();
        quit(t);
    }

    insta::assert_snapshot!(lines);
}

// Keys must stand out from their dimmed descriptions.
#[test]
fn hint_keys_are_bold_and_descriptions_dim() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home, (80, 12));
    let screen = t.snapshot_after(loaded).unwrap();
    quit(t);

    let row = screen.rows() - 1;
    let text = hint_line(&screen);
    let style = |needle: &str| {
        let col = text.find(needle).unwrap();
        *screen
            .cell(row, u16::try_from(col).unwrap())
            .unwrap()
            .style()
    };

    for key in ["j/k", "^d/^u", "enter", "?:"] {
        assert!(style(key).bold, "{key} is not bold");
        assert!(!style(key).dim, "{key} is dim");
    }

    for description in ["move", "scroll", "read", "help"] {
        assert!(style(description).dim, "{description} is not dim");
        assert!(!style(description).bold, "{description} is bold");
    }
}

#[test]
fn the_help_popup_lists_the_keymap_by_group() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    // The tall one has room for every group; the others cut the last.
    let sizes = [
        ("popup_80", (80, 24)),
        ("popup_44", (44, 18)),
        ("popup_tall", (80, 36)),
    ];
    for (name, size) in sizes {
        let mut t = postal(&home, size);
        t.wait_until(loaded).unwrap();
        t.send(Key::Char('?')).unwrap();
        let screen = t.snapshot_after(popup_open).unwrap();
        insta::assert_snapshot!(name, screen);
        t.send(Key::Char('?')).unwrap();
        t.wait_until(|s| !popup_open(s)).unwrap();
        quit(t);
    }
}

// `q` in the popup must not take the whole client down with it.
#[test]
fn q_and_esc_close_the_popup_without_quitting_and_other_keys_are_ignored() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home, (100, 24));
    t.wait_until(|s| loaded(s) && s.contains(" 1/2 ")).unwrap();

    t.send(Key::Char('?')).unwrap();
    t.wait_until(popup_open).unwrap();
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    t.send(Key::Char('q')).unwrap();
    t.wait_until(|s| !popup_open(s)).unwrap();
    assert!(t.screen().contains(" 1/2 "), "{}", t.screen());

    t.send(Key::Char('?')).unwrap();
    t.wait_until(popup_open).unwrap();
    t.send(Key::Esc).unwrap();
    t.wait_until(|s| !popup_open(s)).unwrap();

    // Esc on the main screen must not quit. The pause keeps the terminal
    // from reading Esc and the next key as one Alt chord.
    t.send(Key::Esc).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert!(t.screen().contains(" 1/2 "), "{}", t.screen());
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains(" 2/2 ")).unwrap();
    quit(t);
}

#[test]
fn a_toast_takes_the_right_slot_for_three_seconds() {
    let server = FakeImap::start(two_messages());
    server.stall(Stall::Whole);
    let home = Home::for_server(&server);
    let mut t = postal(&home, (80, 12));
    t.wait_until(|s| loaded(s) && s.contains("Loading…"))
        .unwrap();

    t.send(Key::Char('|')).unwrap();
    let screen = t
        .snapshot_after(|s| hint_line(s).contains("not downloaded"))
        .unwrap();
    let line = hint_line(&screen);
    assert!(line.trim_end().ends_with("not downloaded"), "{line}");
    assert!(line.contains("?: help"), "{line}");

    let shown = std::time::Instant::now();
    t.wait_until(|s| !hint_line(s).contains("not downloaded"))
        .unwrap();
    assert!(shown.elapsed() > Duration::from_secs(2), "gone too soon");
    server.release();
    quit(t);
}
