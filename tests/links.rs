//! `o` labels the links the Preview window shows, and a label opens its URL
//! in the browser, here a recording stand-in for `open` and `xdg-open`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{fmt::Write, time::Duration};

use support::{
    browser::Browser,
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage, Stall},
    home::Home,
};
use termlens::{Color, Key, Screen, Terminal};

/// Postal in the text View, where a label moves the link text right.
fn postal(home: &Home, browser: &Browser) -> Terminal {
    let mut t = spawn(home, browser);
    // Not sent before the first frame: the start reads the terminal's
    // answer to its colour query, and would take the key with it.
    t.wait_until(|s| s.contains(" Inbox ")).unwrap();
    t.send(Key::Char('v')).unwrap();
    t
}

fn spawn(home: &Home, browser: &Browser) -> Terminal {
    Terminal::builder()
        .size(120, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("PATH", browser.path())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn quit(mut t: Terminal) {
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn html(subject: &str, day: u32, html: &str) -> FakeMessage {
    let date = format!("2024-11-{day:02}T09:00:00Z");
    FakeMessage::new("Ada Lovelace", subject, &date).body(FakeBody::text("html", "7bit", html))
}

fn start(messages: Vec<FakeMessage>) -> (FakeImap, Home, Browser) {
    let server = FakeImap::start(messages);
    let home = Home::for_server(&server);
    (server, home, Browser::new())
}

const NOTES: &str = "https://example.com/notes";
const AGENDA: &str = "https://example.com/agenda";

/// Two web links, one of them twice, and a link that must not be opened.
fn two_links() -> String {
    format!(
        "<p>Read <a href=\"{NOTES}\">the notes</a> and <a href=\"{AGENDA}\">the agenda</a>.</p>\
         <p>Again: <a href=\"{NOTES}\">notes</a>. Also <a href=\"file:///etc/passwd\">a file</a>.</p>"
    )
}

/// The style of the first cell of `needle` on screen.
fn style_at(screen: &Screen, needle: &str) -> termlens::Style {
    (0..screen.rows())
        .find_map(|row| {
            let text = screen.row_text(row);
            let (before, _) = text.split_once(needle)?;
            let col = u16::try_from(before.chars().count()).unwrap();
            Some(*screen.cell(row, col).unwrap().style())
        })
        .unwrap()
}

// One label per distinct web URL, before its first text only; the URL of
// the label typed is the one opened.
#[test]
fn a_one_letter_label_opens_its_url() {
    let (_server, home, browser) = start(vec![html("Two links", 18, &two_links())]);
    let mut t = postal(&home, &browser);
    t.wait_until(|s| s.contains("│ Read the notes and the agenda"))
        .unwrap();

    t.send(Key::Char('o')).unwrap();
    let screen = t
        .snapshot_after(|s| s.contains("Read athe notes and sthe agenda"))
        .unwrap();
    assert!(
        screen.contains("Again: notes. Also a file."),
        "{}",
        screen.text()
    );
    let label = style_at(&screen, "athe notes");
    assert_eq!(label.fg, Color::Indexed(3));
    assert!(label.reverse && label.bold, "{label:?}");

    t.send(Key::Char('s')).unwrap();
    browser.wait_for(&[AGENDA]);
    t.wait_until(|s| s.contains("Read the notes and the agenda"))
        .unwrap();
    quit(t);
}

// Past nine links the labels take two letters, all of them, so no label
// is the start of another.
#[test]
fn a_two_letter_label_opens_its_url() {
    let mut body = String::new();
    for i in 0..12 {
        write!(body, "<a href=\"https://example.com/{i}\">link {i}</a><br>").ok();
    }

    let (_server, home, browser) = start(vec![html("Twelve links", 18, &body)]);
    let mut t = postal(&home, &browser);
    t.wait_until(|s| s.contains("link 11")).unwrap();

    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains("aalink 0") && s.contains("sdlink 11"))
        .unwrap();
    t.send(Key::Char('s')).unwrap();
    t.send(Key::Char('d')).unwrap();
    browser.wait_for(&["https://example.com/11"]);
    t.wait_until(|s| s.contains("link 0") && !s.contains("aalink 0"))
        .unwrap();
    quit(t);
}

#[test]
fn links_scrolled_out_of_view_get_no_label() {
    let filler = "<br>filler".repeat(60);
    let body = format!(
        "<a href=\"https://example.com/top\">top link</a>{filler}<br>\
         <a href=\"https://example.com/bottom\">bottom link</a>"
    );
    let (_server, home, browser) = start(vec![html("Long", 18, &body)]);
    let mut t = postal(&home, &browser);
    t.wait_until(|s| s.contains("│ top link")).unwrap();

    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains("atop link")).unwrap();
    t.send(Key::Char('a')).unwrap();
    browser.wait_for(&["https://example.com/top"]);

    for _ in 0..6 {
        t.send(Key::PageDown).unwrap();
    }

    t.wait_until(|s| s.contains("│ bottom link") && !s.contains("│ top link"))
        .unwrap();
    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains("abottom link")).unwrap();
    t.send(Key::Char('a')).unwrap();
    browser.wait_for(&["https://example.com/top", "https://example.com/bottom"]);
    quit(t);
}

// A key that is no label leaves hint mode and does nothing else, even one
// the main screen binds.
#[test]
fn a_key_that_is_no_label_cancels_and_is_consumed() {
    let messages = vec![
        html("Older", 17, "<p>Nothing to see</p>"),
        html("Two links", 18, &two_links()),
    ];
    let (_server, home, browser) = start(messages);
    let mut t = postal(&home, &browser);
    let labelled = |s: &Screen| s.contains("athe notes");
    let unlabelled = |s: &Screen| s.contains(" 1/2 ") && s.contains("│ Read the notes");
    t.wait_until(unlabelled).unwrap();

    // `j` is in the label alphabet, but no label here is `j`.
    for cancel in [Key::Char('j'), Key::Esc, Key::Char('q')] {
        t.send(Key::Char('o')).unwrap();
        t.wait_until(labelled).unwrap();
        t.send(cancel).unwrap();
        t.wait_until(unlabelled).unwrap();
    }

    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains(" 2/2 ")).unwrap();
    quit(t);
    assert!(browser.opened().is_empty(), "{:?}", browser.opened());
}

// Labels belong to the Preview they were found in: once the selected
// Message is expunged under them, typing one must not open its old URL.
#[test]
fn hint_mode_ends_when_the_selected_message_goes() {
    let messages = vec![
        html(
            "Older",
            17,
            "<p><a href=\"https://example.com/older\">older</a></p>",
        ),
        html("Two links", 18, &two_links()),
    ];
    let (server, home, browser) = start(messages);
    let mut t = postal(&home, &browser);
    t.wait_until(|s| s.contains("│ Read the notes")).unwrap();
    quit(t);

    server.stall(Stall::Greeting);
    let mut t = postal(&home, &browser);
    t.wait_until(|s| s.contains("│ Read the notes")).unwrap();
    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains("athe notes")).unwrap();
    server.expunge(FIRST_UID + 1);
    server.release();
    t.wait_until(|s| s.contains(" 1/1 ") && s.contains("│ older"))
        .unwrap();

    t.send(Key::Char('a')).unwrap();
    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains("aolder")).unwrap();
    t.send(Key::Char('a')).unwrap();
    browser.wait_for(&["https://example.com/older"]);
    quit(t);
}

#[test]
fn no_visible_links_toasts_no_links() {
    let (_server, home, browser) = start(vec![html("Plain", 18, "<p>Nothing to see</p>")]);
    let mut t = postal(&home, &browser);
    t.wait_until(|s| s.contains("Nothing to see")).unwrap();
    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains("no links")).unwrap();
    quit(t);
}

// In the rendered View a label uses blank space before its link when
// available, else covers the link's start, so the mail keeps its columns.
#[test]
fn a_label_covers_its_link_in_the_rendered_view() {
    let body = format!(
        "<table width=\"100%\"><tr><td><a href=\"{NOTES}\">the notes</a></td>\
         <td align=\"right\">right column</td></tr></table>"
    );
    let (_server, home, browser) = start(vec![html("Columns", 18, &body)]);
    let mut t = spawn(&home, &browser);
    let row = |s: &Screen| {
        s.rect_text(44..118, 4..29)
            .lines()
            .find(|text| text.contains("right column"))
            .unwrap_or_default()
            .to_owned()
    };

    t.wait_until(|s| row(s).contains("the notes")).unwrap();
    let before = row(&t.screen());

    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| row(s).contains("athe notes")).unwrap();
    let labelled = row(&t.screen());
    assert_eq!(labelled.find("right column"), before.find("right column"));

    t.send(Key::Char('a')).unwrap();
    browser.wait_for(&[NOTES]);
    quit(t);
}

const MEET: &str = "https://meet.google.com/abc-defg-hij";

// The link to join by sits in the invitation's calendar part, which the
// body may not repeat; its line on the card takes a label like the body's.
#[test]
fn the_link_to_join_an_invitation_by_opens_from_its_card() {
    let calendar = format!(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:sync@example.com\r\n\
         DTSTART:20241120T140000Z\r\nDTEND:20241120T150000Z\r\nSUMMARY:Sync\r\n\
         DESCRIPTION:Join with Google Meet: {MEET}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    );
    let body = FakeBody::multipart(
        "alternative",
        vec![
            FakeBody::plain("Talk soon."),
            FakeBody::text("calendar", "7bit", calendar),
        ],
    );
    let date = "2024-11-18T09:00:00Z";
    let invitation = FakeMessage::new("Ada Lovelace", "Invitation: Sync", date).body(body);
    let (_server, home, browser) = start(vec![invitation]);
    let mut t = postal(&home, &browser);
    t.wait_until(|s| s.contains(&format!("Join       {MEET}")) && s.contains("Talk soon."))
        .unwrap();

    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains(&format!("Join       a{MEET}")))
        .unwrap();
    t.send(Key::Char('a')).unwrap();
    browser.wait_for(&[MEET]);
    quit(t);
}
