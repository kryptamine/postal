//! `|` hands the Message to `$PAGER`, and the screen comes back whole.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
    pager::{main_screen, previewed},
};
use termlens::Key;

const HTML: &str = "<p>Read <a href=\"https://example.com/notes\">the notes</a> \
    and <a href=\"https://example.com/agenda\"><b>the agenda</b></a>.</p>\
    <p>See you <i>soon</i>.</p>";

fn message(html: &str) -> FakeMessage {
    let body = FakeBody::text("html", "7bit", html);
    FakeMessage::new("Ada Lovelace", "Weekly notes", "2024-11-18T09:00:00Z").body(body)
}

/// What `$PAGER` read of `html` after `keys`, the escape character made
/// visible, so a snapshot shows the styling.
fn paged(html: &str, keys: &[Key], no_color: bool) -> String {
    let copy = support::pager::paged(message(html), keys, no_color);
    String::from_utf8(copy).unwrap().replace('\x1b', "␛")
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

// With no `$PAGER`, `less` pages the Message even when `$LESS` says to
// quit at once on text that fits the screen, which only flashes it.
#[test]
fn the_default_pager_waits_though_less_would_quit_on_one_screen() {
    let server = FakeImap::start(vec![message(HTML)]);
    let home = Home::for_server(&server);
    let mut t = home
        .terminal()
        .env("LESS", "-FR")
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    t.wait_until(previewed).unwrap();
    t.send(Key::Char('v')).unwrap();
    t.send(Key::Char('|')).unwrap();
    // `-F` would quit before showing the prompt.
    t.wait_until(|s| s.contains("(END)")).unwrap();
    t.send(Key::Char('q')).unwrap();
    t.wait_until(main_screen).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}
