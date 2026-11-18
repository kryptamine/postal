#[allow(dead_code, reason = "each test binary uses part of the harness")]
mod support;

use std::time::Duration;

use chrono::{Datelike, Utc};
use support::{
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

const FETCH: &str = "FETCH 1:* (UID FLAGS INTERNALDATE ENVELOPE X-GM-MSGID BODYSTRUCTURE BODY.PEEK[HEADER.FIELDS (FROM)])";

fn postal(home: &Home) -> Terminal {
    postal_with(home, (80, 25), &[])
}

fn postal_with(home: &Home, (cols, rows): (u16, u16), env: &[(&str, &str)]) -> Terminal {
    let mut builder = Terminal::builder()
        .size(cols, rows)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10));
    for (key, value) in env {
        builder = builder.env(key, value);
    }

    builder.spawn(env!("CARGO_BIN_EXE_postal")).unwrap()
}

/// The subject line of the card drawn in reverse video, the selection.
fn selected_subject(screen: &Screen) -> Option<String> {
    let first =
        (0..screen.rows()).find(|&row| screen.cell(row, 3).is_some_and(|c| c.style().reverse))?;
    let subject = screen.row_text(first + 1);
    Some(
        subject
            .trim_matches(|c: char| c == '│' || c.is_whitespace())
            .to_owned(),
    )
}

fn wait_selected(t: &mut Terminal, subject: &str) {
    t.wait_until(|s| selected_subject(s).as_deref() == Some(subject))
        .unwrap();
}

fn loaded(screen: &Screen) -> bool {
    screen.contains(" unread ")
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

/// Unread with encoded words, long CJK and emoji subjects, and a `From` the
/// server could not parse.
fn varied_messages() -> Vec<FakeMessage> {
    vec![
        FakeMessage::new(
            "=?UTF-8?B?0JDQu9C10LrRgdCw0L3QtNGA?=",
            "=?UTF-8?Q?Caf=C3=A9?= menu\r\n for the week ahead",
            "2024-11-17T14:05:00Z",
        )
        .unread(),
        FakeMessage::new(
            "山田太郎",
            "東京オフィス移転のお知らせ：新しい住所、引っ越しの日程、各部署の座席表、駐車場の利用方法、そして新しい会議室の予約手順について",
            "2024-11-16T09:00:00Z",
        ),
        FakeMessage::new(
            "Events Team",
            "🎉 Launch party on Friday 🍕🍺🍕🍺🍕🍺 for everyone on the team, families and friends welcome 🎈🎈",
            "2024-11-15T09:00:00Z",
        ),
        FakeMessage::new("", "", "2024-11-14T09:00:00Z")
            .unparsable_from("=?UTF-8?Q?Caf=C3=A9_Bot?= <broken"),
    ]
}

#[test]
fn shows_the_newest_messages_first_and_moves_the_selection() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);

    let screen = t.snapshot_after(loaded).unwrap();
    insta::assert_snapshot!(screen);
    assert_eq!(selected_subject(&screen).as_deref(), Some("Newest"));

    t.send(Key::Char('j')).unwrap();
    wait_selected(&mut t, "Second newest");
    t.send(Key::Char('j')).unwrap();
    wait_selected(&mut t, "Third");
    t.send(Key::Char('k')).unwrap();
    wait_selected(&mut t, "Second newest");
    // Only three cards fit, so the last one is reached by scrolling.
    t.send(Key::Char('G')).unwrap();
    wait_selected(&mut t, "Oldest");
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Char('g')).unwrap();
    wait_selected(&mut t, "Newest");
    t.send(Key::Char('k')).unwrap();
    t.send(Key::Down).unwrap();
    wait_selected(&mut t, "Second newest");
    t.send(Key::End).unwrap();
    wait_selected(&mut t, "Oldest");
    t.send(Key::Up).unwrap();
    wait_selected(&mut t, "Third");
    t.send(Key::Home).unwrap();
    wait_selected(&mut t, "Newest");
    quit(t);
}

#[test]
fn reads_the_inbox_without_changing_it() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(loaded).unwrap();
    quit(t);

    let commands = server.commands();
    for expected in ["EXAMINE \"INBOX\"", "UID SEARCH UNSEEN", FETCH] {
        assert!(commands.contains(&expected.to_owned()), "{commands:#?}");
    }

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

fn five_hundred_and_three(unread_outside_the_window: bool) -> Vec<FakeMessage> {
    (0..502)
        .map(|i| {
            let date = chrono::DateTime::UNIX_EPOCH + chrono::Duration::hours(i);
            let message =
                FakeMessage::new("Sender", &format!("Message {}", i + 1), &date.to_rfc3339());
            if unread_outside_the_window && i < 3 {
                message.unread()
            } else {
                message
            }
        })
        .chain([FakeMessage::new("Latest", "The very latest", "2024-06-01T00:00:00Z").unread()])
        .collect()
}

#[test]
fn fetches_only_the_newest_500() {
    let server = FakeImap::start(five_hundred_and_three(false));
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.row_text(2).contains("The very latest"))
        .unwrap();
    quit(t);

    let fetches: Vec<_> = server
        .commands()
        .into_iter()
        .filter(|c| c.starts_with("FETCH"))
        .collect();
    assert_eq!(fetches, [FETCH.replace("1:*", "4:*")]);
}

#[test]
fn counts_unread_over_the_whole_inbox() {
    // The three oldest are Unread but outside the fetched window: a count
    // over the rows would say 1.
    let server = FakeImap::start(five_hundred_and_three(true));
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    let screen = t.snapshot_after(loaded).unwrap();
    quit(t);

    assert!(screen.row_text(0).contains(" Inbox  4 unread "), "{screen}");
}

#[test]
fn draws_cards_at_each_width() {
    let server = FakeImap::start(varied_messages());
    let home = Home::for_server(&server);
    for (name, cols) in [("width_100", 100), ("width_28", 28)] {
        let mut t = postal_with(&home, (cols, 32), &[]);
        let screen = t.snapshot_after(loaded).unwrap();
        insta::assert_snapshot!(name, screen);
        quit(t);
    }

    let mut t = postal_with(&home, (19, 14), &[]);
    let screen = t
        .snapshot_after(|s| s.contains("terminal too small"))
        .unwrap();
    insta::assert_snapshot!("width_19", screen);
    quit(t);
}

#[test]
fn shows_the_three_date_forms() {
    let today = Utc::now().date_naive();
    // On 1 January no earlier day of this year exists to show "1 Jan".
    if today.ordinal() == 1 {
        return;
    }

    let server = FakeImap::start(vec![
        FakeMessage::new("Today", "Shows the time", &format!("{today}T00:01:00Z")),
        FakeMessage::new(
            "This year",
            "Shows day and month",
            &format!("{}-01-01T00:00:00Z", today.year()),
        ),
        FakeMessage::new("Older", "Shows the full date", "2024-11-17T12:00:00Z"),
    ]);
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    let screen = t.snapshot_after(loaded).unwrap();
    insta::assert_snapshot!(screen);
    quit(t);
}

#[test]
fn shows_an_empty_inbox() {
    let server = FakeImap::start(Vec::new());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    let screen = t.snapshot_after(|s| s.contains("Inbox is empty")).unwrap();
    insta::assert_snapshot!(screen);
    quit(t);
    assert!(!server.commands().iter().any(|c| c.starts_with("FETCH")));
}

#[test]
fn ctrl_c_quits() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(loaded).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[test]
fn colours_unless_no_color_is_set() {
    let server = FakeImap::start(four_messages());
    let home = Home::for_server(&server);

    let mut t = postal(&home);
    let coloured = t.snapshot_after(loaded).unwrap();
    quit(t);
    let mut t = postal_with(&home, (80, 25), &[("NO_COLOR", "1")]);
    let plain = t.snapshot_after(loaded).unwrap();
    quit(t);

    let style = |s: &Screen, row, col| *s.cell(row, col).unwrap().style();
    assert_eq!(style(&coloured, 1, 4).fg, termlens::Color::Indexed(4));
    for screen in [&coloured, &plain] {
        let newest = style(screen, 1, 4);
        assert!(newest.reverse, "the selection survives NO_COLOR");
        assert!(newest.bold, "the Unread card stays bold");
        assert_eq!(screen.cell(1, 2).unwrap().contents(), "●");
        // Card 2 is read and unselected; its date ends one column of padding
        // short of the right border.
        assert!(style(screen, 4, 77).dim, "the date is muted");
        assert!(style(screen, 0, 0).dim, "the border is muted");
        assert!(style(screen, 0, 2).bold, "the title is bold");
    }

    let plain_cells = (0..plain.rows()).flat_map(|r| (0..plain.cols()).map(move |c| (r, c)));
    for (row, col) in plain_cells {
        let style = plain.cell(row, col).unwrap().style();
        assert_eq!(style.fg, termlens::Color::Default, "colour at {row},{col}");
        assert_eq!(style.bg, termlens::Color::Default, "colour at {row},{col}");
    }
}
