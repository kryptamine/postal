//! `|` hands the Message to `$PAGER`, and the screen comes back whole.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::path::Path;

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

const HTML: &str = "<p>Read <a href=\"https://example.com/notes\">the notes</a> \
    and <a href=\"https://example.com/agenda\"><b>the agenda</b></a>.</p>\
    <p>See you <i>soon</i>.</p>";

fn postal(home: &Home, envs: &[(&str, &Path)], no_color: bool) -> Terminal {
    let mut builder = home.terminal();
    for (key, value) in envs {
        builder = builder.env(key, value);
    }

    if no_color {
        builder = builder.env("NO_COLOR", "1");
    }

    builder.spawn(env!("CARGO_BIN_EXE_postal")).unwrap()
}

fn server_with(html: &str) -> FakeImap {
    let body = FakeBody::text("html", "7bit", html);
    let message = FakeMessage::new("Ada Lovelace", "Weekly notes", "2024-11-18T09:00:00Z");
    FakeImap::start(vec![message.body(body)])
}

// The list's snippet shows the text before the body is cached; the Preview's
// header comes with the body, and only then does `|` find it.
fn main_screen(screen: &Screen) -> bool {
    screen.alternate_screen() && screen.contains(" 1/1 ") && screen.contains("Subject Weekly notes")
}

/// The escape character made visible, so a snapshot shows the styling.
fn visible(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec())
        .unwrap()
        .replace('\x1b', "␛")
}

/// What a recording `$PAGER` read of `html` after `keys`, once the TUI
/// took the screen back.
fn paged(html: &str, keys: &[Key], no_color: bool) -> String {
    let server = server_with(html);
    let home = Home::for_server(&server);
    let dir = tempfile::tempdir().unwrap();
    let tmp = dir.path().join("tmp");
    std::fs::create_dir(&tmp).unwrap();
    let record = dir.path().join("paged");
    // Holds the pager open until the test answers on the terminal, so the
    // suspended screen can be seen.
    let pager = format!(
        "cat > '{}'; printf 'pager done'; read answer < /dev/tty",
        record.display()
    );
    let mut t = postal(
        &home,
        &[("PAGER", Path::new(&pager)), ("TMPDIR", &tmp)],
        no_color,
    );
    t.wait_until(main_screen).unwrap();

    for key in keys {
        t.send(*key).unwrap();
    }

    t.wait_until(|s| !s.alternate_screen() && s.contains("pager done"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(main_screen).unwrap();
    // The TUI still takes keys after the pager.
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let written: Vec<_> = std::fs::read_dir(&tmp).unwrap().collect();
    assert!(written.is_empty(), "{written:?}");
    visible(&std::fs::read(&record).unwrap())
}

const TEXT_VIEW: [Key; 2] = [Key::Char('v'), Key::Char('|')];

// The pager copy keeps the Preview's styling, makes each link a hyperlink
// a terminal opens on click, and lists every URL for terminals without.
#[test]
fn pipe_hands_the_message_with_hyperlinks_and_footnotes_to_the_pager() {
    insta::assert_snapshot!("colour", paged(HTML, &TEXT_VIEW, false));
}

#[test]
fn no_color_strips_the_colours_from_the_pager_copy() {
    insta::assert_snapshot!("no_color", paged(HTML, &TEXT_VIEW, true));
}

// The rendered View goes to the pager laid out, its links still hyperlinks.
#[test]
fn the_rendered_view_goes_to_the_pager_laid_out() {
    insta::assert_snapshot!("rendered", paged(HTML, &[Key::Char('|')], false));
}

#[test]
fn the_full_screen_message_goes_to_the_pager_too() {
    let keys = [Key::Enter, Key::Char('|')];
    assert_eq!(
        paged(HTML, &keys, true),
        paged(HTML, &[Key::Char('|')], true)
    );
}

// Mail is untrusted: a control character in a link would end the OSC 8
// sequence early and reach the terminal as a command of its own.
#[test]
fn a_link_with_a_control_character_is_no_hyperlink() {
    let html = "<p><a href=\"https://evil.example/&#27;]2;owned&#7;\">bait</a> \
        <a href=\"https://good.example/\">the agenda</a></p>";
    let copy = paged(html, &[Key::Char('|')], true);
    assert!(copy.contains("␛]8;;https://good.example/"), "{copy}");
    assert!(!copy.contains("␛]8;;https://evil.example"), "{copy}");
}
