//! Nerd Font icons: `icons = "auto"` draws them only in a terminal that
//! bundles the symbols, `nerd` always, `none` never.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{fs, time::Duration};

use support::{
    fake_imap::{EMAIL, FakeImap, FakeMessage, Outage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
    picker::picked,
};
use termlens::{Key, Screen, Terminal};

const UNREAD: &str = "\u{f01ee}";
const OUTBOX: &str = "\u{f048a}";
const OFFLINE: &str = "\u{f0319}";

fn home(icons: &str, imap: &FakeImap, smtp: &FakeSmtp) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\n{icons}\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[smtp]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        imap.port,
        smtp.port,
        fake_oauth::shared().config()
    ))
}

/// Run under `terminal` as `TERM_PROGRAM`, or none; `$EDITOR` writes a
/// Draft to Bob.
fn postal(home: &Home, terminal: Option<&str>) -> Terminal {
    let next = home.xdg_config_home().join("next");
    fs::write(&next, "To: bob@example.com\nSubject: Plans\n\nHi Bob\n").unwrap();
    let builder = Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("EDITOR", format!("cp '{}'", next.display()))
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10));
    let builder = match terminal {
        Some(terminal) => builder.env("TERM_PROGRAM", terminal),
        None => builder,
    };

    builder.spawn(env!("CARGO_BIN_EXE_postal")).unwrap()
}

fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn inbox() -> FakeImap {
    let server = FakeImap::start(vec![
        FakeMessage::new("Ada", "Weekly notes", "2024-11-18T09:00:00Z").unread(),
    ]);
    server.add_folder("[Gmail]/Trash", &["\\Trash"], vec![]);
    server.add_folder("Work", &[], vec![]);
    server
}

/// The Folder Picker's rows, once every Folder is listed.
fn picker_rows(t: &mut Terminal) -> (Vec<String>, Screen) {
    t.send(Key::Char('f')).unwrap();
    let screen = t
        .snapshot_after(|s| picked(s).iter().any(|row| row.ends_with("Work")))
        .unwrap();
    (picked(&screen), screen)
}

#[test]
fn auto_draws_icons_in_a_terminal_that_bundles_them() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home("", &imap, &smtp);
    let mut t = postal(&home, Some("ghostty"));
    let screen = t.snapshot_after(|s| s.contains("Weekly notes")).unwrap();
    assert!(screen.contains(UNREAD) && !screen.contains("●"), "{screen}");

    let (rows, screen) = picker_rows(&mut t);
    assert_eq!(
        rows,
        [
            "\u{f0687} Inbox 1",
            "\u{f0322} Local Drafts",
            "\u{f01b4} Trash",
            "\u{f024b} Work",
        ],
        "{screen}"
    );
    quit(t);
}

// Without a known terminal nothing tells whether the font has the glyphs,
// and a missing glyph draws as a box.
#[test]
fn auto_keeps_the_plain_look_elsewhere_and_none_keeps_it_everywhere() {
    for (icons, terminal) in [
        ("", Some("iTerm.app")),
        ("icons = \"none\"", Some("ghostty")),
    ] {
        let (imap, smtp) = (inbox(), FakeSmtp::start());
        let home = home(icons, &imap, &smtp);
        let mut t = postal(&home, terminal);
        let screen = t.snapshot_after(|s| s.contains("Weekly notes")).unwrap();
        assert!(screen.contains("●") && !screen.contains(UNREAD), "{screen}");

        let (rows, screen) = picker_rows(&mut t);
        assert_eq!(
            rows,
            ["Inbox 1", "Local Drafts", "Trash", "Work"],
            "{screen}"
        );
        quit(t);
    }
}

#[test]
fn nerd_marks_an_outbox_message_and_an_outage() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    smtp.down();
    let home = home("icons = \"nerd\"", &imap, &smtp);
    let mut t = postal(&home, None);
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains("y send")).unwrap();
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("1 outbox")).unwrap();

    t.send(Key::Char('f')).unwrap();
    t.send_str("local").unwrap();
    t.wait_until(|s| {
        picked(s)
            .first()
            .is_some_and(|f| f.ends_with("Local Drafts"))
    })
    .unwrap();
    t.send(Key::Enter).unwrap();
    let screen = t
        .snapshot_after(|s| s.contains(" Local Drafts ") && s.contains(OUTBOX))
        .unwrap();
    assert!(!screen.contains("Outbox "), "{screen}");
    quit(t);

    imap.fail(Outage::Offline);
    let mut t = postal(&home, None);
    t.wait_until(|s| {
        let hint = s.row_text(s.rows() - 1);
        hint.contains(&format!("{OFFLINE} offline · retry in"))
    })
    .unwrap();
    quit(t);
}
