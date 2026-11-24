//! The screen of a Search, and Postal started for one.

use std::time::{Duration, Instant};

use termlens::{Key, Screen, Terminal};

use super::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
    picker::{picked, picker},
};

/// The Message List's columns beside the Preview at 120 columns.
pub const LIST: usize = 42;

pub fn postal(home: &Home) -> Terminal {
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

pub fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

pub fn message(sender: &'static str, subject: &str, date: &str, body: &str) -> FakeMessage {
    FakeMessage::new(sender, subject, date).body(FakeBody::plain(body))
}

/// Newest first: Carol, Alice, Dave, Bob, then the two digests.
pub fn messages() -> Vec<FakeMessage> {
    vec![
        message(
            "Alice",
            "Invoice 1042",
            "2024-03-02T09:00:00Z",
            "Please pay by Friday",
        ),
        message(
            "Bob",
            "Holiday photos",
            "2023-12-31T23:59:00Z",
            "From the beach",
        ),
        message(
            "Carol",
            "Quarterly report",
            "2024-05-10T08:30:00Z",
            "Pages and a yearly summary, pre-payment due",
        ),
        message(
            "Dave",
            "Lunch",
            "2024-01-15T12:00:00Z",
            "The invoice is attached",
        ),
        message("News", "Weekly digest", "2023-11-01T09:00:00Z", "Edition 1"),
        message("News", "Weekly digest", "2023-11-08T09:00:00Z", "Edition 2"),
    ]
}

/// Started, with every Snippet in and the sync idling.
pub fn ready(server: &FakeImap, home: &Home) -> Terminal {
    let mut t = postal(home);
    t.wait_until(|s| s.contains("Pages and a yearly")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !server.commands().iter().any(|c| c == "IDLE") {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }

    t
}

/// The Message List's rows, top to bottom.
pub fn list(screen: &Screen) -> String {
    (0..screen.rows())
        .map(|row| screen.row_text(row).chars().take(LIST).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether the list shows `a` above `b`.
pub fn above(screen: &Screen, a: &str, b: &str) -> bool {
    let list = list(screen);
    list.find(a).zip(list.find(b)).is_some_and(|(a, b)| a < b)
}

/// The subject line of the card drawn in reverse video, the selection.
pub fn selected_subject(screen: &Screen) -> Option<String> {
    let first =
        (0..screen.rows()).find(|&row| screen.cell(row, 3).is_some_and(|c| c.style().reverse))?;
    let subject = screen.row_text(first + 1);
    let subject = subject.split('│').nth(1)?;
    Some(subject.trim().to_owned())
}

pub fn wait_selected(t: &mut Terminal, subject: &str) -> Screen {
    t.snapshot_after(|s| selected_subject(s).as_deref() == Some(subject))
        .unwrap()
}

pub fn hint_line(screen: &Screen) -> String {
    screen.row_text(screen.rows() - 1)
}

/// Shows the Folder the Folder Picker matches to `query`, and waits until
/// its rows are in.
pub fn show(t: &mut Terminal, query: &str, title: &str, row: &str) {
    t.send(Key::Char('f')).unwrap();
    t.wait_until(|s| !picker(s).is_empty()).unwrap();
    t.send_str(query).unwrap();
    t.snapshot_after(|s| {
        picker(s).first().is_some_and(|q| q.ends_with(query)) && !picked(s).is_empty()
    })
    .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.row_text(0).contains(title) && list(s).contains(row))
        .unwrap();
}
