//! `E` opens the Message read-only in `$EDITOR`, and the file goes once the
//! editor exits.

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
use termlens::{Key, Screen};

const HTML: &str = "<p>Read <a href=\"https://example.com/notes\">the notes</a>.</p>\
    <p>See you <i>soon</i>.</p>";

fn main_screen(screen: &Screen) -> bool {
    screen.alternate_screen() && screen.contains(" 1/1 ") && screen.contains("Weekly notes")
}

/// What a recording `$EDITOR` was handed after `keys`, and whether it could
/// write the file; the Message's file is checked gone once the TUI is back.
fn opened(body: FakeBody, keys: &[Key]) -> (String, bool) {
    let message = FakeMessage::new("Ada Lovelace", "Weekly notes", "2024-11-18T09:00:00Z")
        .to("Charles Babbage", "charles@example.com")
        .cc("Mary Somerville", "mary@example.com")
        .body(body);
    let server = FakeImap::start(vec![message]);
    let home = Home::for_server(&server);
    let dir = tempfile::tempdir().unwrap();
    let record = dir.path().join("opened");
    let writable = dir.path().join("writable");
    // Holds the editor open until the test answers on the terminal, so the
    // file can be seen while it is open.
    let editor = format!(
        "sh -c 'cp \"$1\" {record}; test -w \"$1\" && touch {writable}; \
         printf \"editor done\"; read answer < /dev/tty' sh",
        record = record.display(),
        writable = writable.display(),
    );
    let mut t = home
        .terminal()
        .env("EDITOR", Path::new(&editor))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    t.wait_until(main_screen).unwrap();

    for key in keys {
        t.send(*key).unwrap();
    }

    t.wait_until(|s| !s.alternate_screen() && s.contains("editor done"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(main_screen).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let messages = home.xdg_cache_home().join("postal/messages");
    let left: Vec<_> = std::fs::read_dir(&messages).unwrap().collect();
    assert!(left.is_empty(), "{left:?}");
    (std::fs::read_to_string(&record).unwrap(), writable.exists())
}

// The header block and the body as text, each URL listed under it, so the
// editor's search and an agent both find what the Preview shows.
#[test]
fn e_opens_the_message_as_text_in_the_editor() {
    let (text, _) = opened(FakeBody::text("html", "7bit", HTML), &[Key::Char('E')]);
    insta::assert_snapshot!(text);
}

// Saving it would change nothing in the Cache, so the editor says it cannot.
#[test]
fn the_message_file_is_read_only() {
    let (_, writable) = opened(FakeBody::plain("Hello"), &[Key::Char('E')]);
    assert!(!writable);
}

#[test]
fn reading_opens_the_message_at_the_cursor() {
    let (text, _) = opened(FakeBody::plain("Hello"), &[Key::Enter, Key::Char('E')]);
    assert!(text.ends_with("\n\nHello\n"), "{text}");
}

// A Quote the Preview folds away is part of what is searched and asked.
#[test]
fn the_quote_is_kept() {
    let body = "Sounds good.\n\nOn Mon, Ada wrote:\n> Shall we meet?\n";
    let (text, _) = opened(FakeBody::plain(body), &[Key::Char('E')]);
    assert!(text.contains("> Shall we meet?"), "{text}");
}
