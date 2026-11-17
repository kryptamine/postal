#[allow(dead_code, reason = "each test binary uses part of the harness")]
mod support;

use std::time::Duration;

use support::{
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

fn postal(home: &Home) -> Terminal {
    postal_with(home, &[])
}

fn postal_with(home: &Home, env: &[(&str, &str)]) -> Terminal {
    let mut builder = Terminal::builder()
        .size(80, 12)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10));
    for (key, value) in env {
        builder = builder.env(key, value);
    }

    builder.spawn(env!("CARGO_BIN_EXE_postal")).unwrap()
}

/// The row drawn in reverse video, the selection.
fn selected_row(screen: &Screen) -> Option<u16> {
    (0..screen.rows()).find(|&row| screen.cell(row, 4).is_some_and(|c| c.style().reverse))
}

fn wait_selected(t: &mut Terminal, row: u16) {
    t.wait_until(|s| selected_row(s) == Some(row)).unwrap();
}

fn quit(mut t: Terminal) {
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn four_messages() -> Vec<FakeMessage> {
    // Sequence order differs from arrival order: the list must sort by INTERNALDATE.
    vec![
        FakeMessage::new("Alice", "Second newest", "2024-03-02T09:00:00Z"),
        FakeMessage::new("Bob", "Oldest", "2023-12-31T23:59:00Z"),
        FakeMessage::new("Carol", "Newest", "2024-05-10T08:30:00Z").unread(),
        FakeMessage::new("Dave", "Third", "2024-01-15T12:00:00Z"),
    ]
}

#[test]
fn shows_the_newest_messages_first_and_moves_the_selection() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);

    let screen = t.snapshot_after(|s| s.contains("Oldest")).unwrap();
    insta::assert_snapshot!(screen);
    assert_eq!(selected_row(&screen), Some(0));

    t.send(Key::Char('j')).unwrap();
    wait_selected(&mut t, 1);
    t.send(Key::Char('j')).unwrap();
    wait_selected(&mut t, 2);
    t.send(Key::Char('k')).unwrap();
    wait_selected(&mut t, 1);
    t.send(Key::Char('G')).unwrap();
    wait_selected(&mut t, 3);
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Char('g')).unwrap();
    wait_selected(&mut t, 0);
    t.send(Key::Char('k')).unwrap();
    t.send(Key::Down).unwrap();
    wait_selected(&mut t, 1);
    t.send(Key::End).unwrap();
    wait_selected(&mut t, 3);
    t.send(Key::Up).unwrap();
    wait_selected(&mut t, 2);
    t.send(Key::Home).unwrap();
    wait_selected(&mut t, 0);
    quit(t);
}

#[test]
fn reads_the_inbox_without_changing_it() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Oldest")).unwrap();
    quit(t);

    let commands = server.commands();
    assert!(
        commands.contains(&"EXAMINE INBOX".to_owned()),
        "{commands:#?}"
    );
    assert!(
        commands.contains(&"FETCH 1:* (UID FLAGS INTERNALDATE ENVELOPE)".to_owned()),
        "{commands:#?}"
    );
    // Postal is read-only: a SELECT would let the server clear \Recent.
    for forbidden in [
        "SELECT",
        "STORE",
        "EXPUNGE",
        "COPY",
        "UID STORE",
        "UID COPY",
    ] {
        assert!(
            !commands
                .iter()
                .any(|c| c.starts_with(&format!("{forbidden} ")) || c == forbidden),
            "sent {forbidden}: {commands:#?}"
        );
    }
}

#[test]
fn fetches_only_the_newest_500() {
    let messages: Vec<_> = (0..502)
        .map(|i| {
            let date = chrono::DateTime::UNIX_EPOCH + chrono::Duration::hours(i);
            FakeMessage::new("Sender", &format!("Message {}", i + 1), &date.to_rfc3339())
        })
        .chain([FakeMessage::new(
            "Latest",
            "The very latest",
            "2024-06-01T00:00:00Z",
        )])
        .collect();
    let server = FakeImap::start(messages);
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.row_text(0).contains("The very latest"))
        .unwrap();
    quit(t);

    let fetches: Vec<_> = server
        .commands()
        .into_iter()
        .filter(|c| c.starts_with("FETCH"))
        .collect();
    assert_eq!(fetches, ["FETCH 4:* (UID FLAGS INTERNALDATE ENVELOPE)"]);
}

#[test]
fn decodes_and_truncates_rows_to_one_line() {
    let server = FakeImap::start(vec![
        FakeMessage::new(
            "=?UTF-8?B?0JDQu9C10LrRgdCw0L3QtNGA?=",
            "=?UTF-8?Q?Caf=C3=A9?= menu\r\n for the week ahead, with every dish we plan to serve",
            "2024-02-01T10:00:00Z",
        ),
        FakeMessage {
            from_name: None,
            ..FakeMessage::new("", "", "2024-01-01T10:00:00Z")
        },
    ]);
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    let screen = t.snapshot_after(|s| s.contains("(no subject)")).unwrap();
    insta::assert_snapshot!(screen);
    quit(t);
}

#[test]
fn shows_an_empty_inbox() {
    let server = FakeImap::start(Vec::new());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Inbox is empty")).unwrap();
    quit(t);
    assert!(!server.commands().iter().any(|c| c.starts_with("FETCH")));
}

#[test]
fn ctrl_c_quits() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Oldest")).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[test]
fn colours_unless_no_color_is_set() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);

    let mut t = postal(&home);
    let coloured = t.snapshot_after(|s| s.contains("Oldest")).unwrap();
    quit(t);
    let mut t = postal_with(&home, &[("NO_COLOR", "1")]);
    let plain = t.snapshot_after(|s| s.contains("Oldest")).unwrap();
    quit(t);

    // Row 1 is a read, unselected Message; its date sits in the last columns.
    let date = |s: &Screen| *s.cell(1, 79).unwrap().style();
    assert_ne!(date(&coloured).fg, date(&plain).fg);
    for screen in [&coloured, &plain] {
        let newest = screen.cell(0, 4).unwrap().style();
        assert!(newest.reverse, "the selection survives NO_COLOR");
        assert!(newest.bold, "the Unread row stays bold");
        assert_eq!(screen.cell(0, 0).unwrap().contents(), "●");
    }

    let plain_cells = (0..plain.rows()).flat_map(|r| (0..plain.cols()).map(move |c| (r, c)));
    for (row, col) in plain_cells {
        let style = plain.cell(row, col).unwrap().style();
        assert_eq!(style.fg, termlens::Color::Default, "colour at {row},{col}");
        assert_eq!(style.bg, termlens::Color::Default, "colour at {row},{col}");
    }
}

#[test]
fn a_rejected_login_exits_with_the_reason() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    std::fs::write(home.xdg_config_home().join("postal/secret"), "wrong").unwrap();
    let mut t = postal(&home);
    let status = t.wait_exit().unwrap();
    assert!(!status.success());
    assert!(t.screen().contains("Invalid credentials"), "{}", t.screen());
}
