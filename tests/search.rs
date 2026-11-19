//! Search: `/` narrows the Message List to the Messages a fuzzy Query
//! matches, best match first, typed in the hint line.

mod support;

use std::time::{Duration, Instant};

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Color, Key, Screen, Terminal};

/// The Message List's columns beside the Preview at 120 columns.
const LIST: usize = 42;

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
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn message(sender: &'static str, subject: &str, date: &str, body: &str) -> FakeMessage {
    FakeMessage::new(sender, subject, date).body(FakeBody::plain(body))
}

/// Newest first: Carol, Alice, Dave, Bob, then the two digests.
fn messages() -> Vec<FakeMessage> {
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
            "Pages and a yearly summary",
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
fn ready(server: &FakeImap, home: &Home) -> Terminal {
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
fn list(screen: &Screen) -> String {
    (0..screen.rows())
        .map(|row| screen.row_text(row).chars().take(LIST).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether the list shows `a` above `b`.
fn above(screen: &Screen, a: &str, b: &str) -> bool {
    let list = list(screen);
    list.find(a).zip(list.find(b)).is_some_and(|(a, b)| a < b)
}

/// The subject line of the card drawn in reverse video, the selection.
fn selected_subject(screen: &Screen) -> Option<String> {
    let first =
        (0..screen.rows()).find(|&row| screen.cell(row, 3).is_some_and(|c| c.style().reverse))?;
    let subject = screen.row_text(first + 1);
    let subject = subject.split('│').nth(1)?;
    Some(subject.trim().to_owned())
}

fn wait_selected(t: &mut Terminal, subject: &str) -> Screen {
    t.snapshot_after(|s| selected_subject(s).as_deref() == Some(subject))
        .unwrap()
}

fn hint_line(screen: &Screen) -> String {
    screen.row_text(screen.rows() - 1)
}

#[test]
fn a_query_narrows_the_list_best_match_first_and_the_preview_follows() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('/')).unwrap();
    t.send_str("pay").unwrap();
    let screen = t
        .snapshot_after(|s| !list(s).contains("Holiday") && s.contains("Please pay by Friday"))
        .unwrap();
    // `pay` whole in Alice's Snippet beats Carol's scattered P, a and y,
    // though Carol's Message is newer.
    assert!(
        above(&screen, "Invoice 1042", "Quarterly report"),
        "{screen}"
    );
    for gone in ["Lunch", "Weekly digest"] {
        assert!(!list(&screen).contains(gone), "{gone}: {screen}");
    }

    assert_eq!(selected_subject(&screen).as_deref(), Some("Invoice 1042"));
    assert!(
        hint_line(&screen).trim_start().starts_with("/pay"),
        "{screen}"
    );

    // `^j` arrives as `\n`, which a raw terminal must not read as Enter.
    t.send(Key::Ctrl('j')).unwrap();
    let screen = wait_selected(&mut t, "Quarterly report");
    assert!(screen.contains("Pages and a yearly summary"), "{screen}");
    let row = (0..screen.rows())
        .find(|&row| screen.row_text(row).contains("Please pay by"))
        .unwrap();
    let text = screen.row_text(row);
    let col = u16::try_from(text.split_once("pay by").unwrap().0.chars().count()).unwrap();
    for offset in 0..3 {
        let style = screen.cell(row, col + offset).unwrap().style();
        assert!(
            style.bold && style.fg == Color::Indexed(2),
            "{offset}: {style:?}"
        );
    }

    t.send(Key::Ctrl('k')).unwrap();
    wait_selected(&mut t, "Invoice 1042");

    // Enter keeps the Query; the list keys work on the matches.
    t.send(Key::Enter).unwrap();
    let screen = t
        .snapshot_after(|s| hint_line(s).contains("search"))
        .unwrap();
    assert!(hint_line(&screen).contains("/pay"), "{screen}");
    t.send(Key::Char('j')).unwrap();
    wait_selected(&mut t, "Quarterly report");
    t.send(Key::Char('j')).unwrap();
    wait_selected(&mut t, "Quarterly report");

    // Esc ends the Search on the Message selected in it.
    t.send(Key::Esc).unwrap();
    let screen = t
        .snapshot_after(|s| list(s).contains("Holiday photos"))
        .unwrap();
    assert_eq!(
        selected_subject(&screen).as_deref(),
        Some("Quarterly report")
    );
    assert!(!hint_line(&screen).contains("/pay"), "{screen}");
    quit(t);
}

#[test]
fn escape_while_typing_restores_the_selection_held_before_the_search() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Char('j')).unwrap();
    wait_selected(&mut t, "Lunch");

    t.send(Key::Char('/')).unwrap();
    t.send_str("pay").unwrap();
    wait_selected(&mut t, "Invoice 1042");
    t.send(Key::Esc).unwrap();
    let screen = wait_selected(&mut t, "Lunch");
    assert!(list(&screen).contains("Holiday photos"), "{screen}");
    assert!(screen.contains("The invoice is attached"), "{screen}");

    // `/` on a kept Query edits it; Esc then comes back to that `/`.
    t.send(Key::Char('/')).unwrap();
    t.send_str("pay").unwrap();
    t.send(Key::Enter).unwrap();
    t.send(Key::Char('j')).unwrap();
    wait_selected(&mut t, "Quarterly report");
    t.send(Key::Char('/')).unwrap();
    t.send(Key::Char('s')).unwrap();
    t.wait_until(|s| hint_line(s).contains("/pays")).unwrap();
    t.send(Key::Esc).unwrap();
    let screen = t
        .snapshot_after(|s| list(s).contains("Holiday photos"))
        .unwrap();
    assert_eq!(
        selected_subject(&screen).as_deref(),
        Some("Quarterly report")
    );
    quit(t);
}

#[test]
fn a_query_matching_nothing_says_so_and_leaves_the_preview_empty() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('/')).unwrap();
    t.send_str("zqx").unwrap();
    let screen = t.snapshot_after(|s| s.contains("no matches")).unwrap();
    assert!(!screen.contains("From "), "{screen}");
    assert!(!screen.contains("Loading"), "{screen}");

    // An empty Query is the whole list again, newest first.
    for _ in 0..3 {
        t.send(Key::Backspace).unwrap();
    }

    let screen = wait_selected(&mut t, "Quarterly report");
    assert!(
        above(&screen, "Quarterly report", "Invoice 1042"),
        "{screen}"
    );
    assert!(above(&screen, "Invoice 1042", "Lunch"), "{screen}");
    quit(t);
}

#[test]
fn an_uppercase_letter_makes_the_query_case_sensitive() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('/')).unwrap();
    t.send_str("inv").unwrap();
    let screen = t
        .snapshot_after(|s| list(s).contains("Lunch") && list(s).contains("Invoice"))
        .unwrap();
    assert!(!list(&screen).contains("Holiday"), "{screen}");

    t.send(Key::Backspace).unwrap();
    t.send(Key::Backspace).unwrap();
    t.send(Key::Backspace).unwrap();
    t.send_str("Inv").unwrap();
    let screen = t
        .snapshot_after(|s| !list(s).contains("Lunch") && list(s).contains("Invoice"))
        .unwrap();
    assert!(!list(&screen).contains("Holiday"), "{screen}");
    quit(t);
}

#[test]
fn a_tie_goes_to_the_newer_message() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('/')).unwrap();
    t.send_str("digest").unwrap();
    let screen = t
        .snapshot_after(|s| !list(s).contains("Lunch") && list(s).contains("Edition 1"))
        .unwrap();
    assert!(above(&screen, "Edition 2", "Edition 1"), "{screen}");
    quit(t);
}

#[test]
fn a_new_message_that_matches_joins_the_results_and_the_selection_stays() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('/')).unwrap();
    t.send_str("pay").unwrap();
    wait_selected(&mut t, "Invoice 1042");
    t.send(Key::Ctrl('j')).unwrap();
    wait_selected(&mut t, "Quarterly report");

    server.deliver(message("Frank", "Hello", "2024-06-02T00:00:00Z", "Hi"));
    server.deliver(message(
        "Erin",
        "Payslip",
        "2024-06-01T00:00:00Z",
        "For May",
    ));
    let screen = t.snapshot_after(|s| list(s).contains("Payslip")).unwrap();
    assert!(!list(&screen).contains("Hello"), "{screen}");
    assert_eq!(
        selected_subject(&screen).as_deref(),
        Some("Quarterly report")
    );
    // Still typing: the Query takes the next character.
    t.send(Key::Char('s')).unwrap();
    let screen = t.snapshot_after(|s| !list(s).contains("Invoice")).unwrap();
    assert!(hint_line(&screen).contains("/pays"), "{screen}");
    assert_eq!(selected_subject(&screen).as_deref(), Some("Payslip"));
    quit(t);
}
