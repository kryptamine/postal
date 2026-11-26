//! `B` writes the Message as a page and hands it to the desktop's opener,
//! here a recording stand-in for `open` and `xdg-open`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::fs;

use support::{
    browser::Browser,
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen};

fn main_screen(screen: &Screen) -> bool {
    screen.alternate_screen() && screen.contains(" 1/1 ") && screen.contains("Weekly notes")
}

/// The page the opener was handed after `keys`; its file is the one
/// emptied as the next TUI starts.
fn opened(body: FakeBody, keys: &[Key]) -> String {
    let message =
        FakeMessage::new("Ada Lovelace", "Weekly notes", "2024-11-18T09:00:00Z").body(body);
    let server = FakeImap::start(vec![message]);
    let home = Home::for_server(&server);
    let browser = Browser::new();
    let mut t = home
        .terminal()
        .env("PATH", browser.path())
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    t.wait_until(main_screen).unwrap();

    for key in keys {
        t.send(*key).unwrap();
    }

    t.wait_until(|s| s.contains("opened in the browser"))
        .unwrap();
    let path = browser.wait_any();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let page = fs::read_to_string(&path).unwrap();
    assert!(path.starts_with(home.xdg_cache_home().join("postal").to_str().unwrap()));
    page
}

// The page is the HTML as sent, so the browser shows what the cell grid
// cannot, and its scripts never run though it opens from disk.
#[test]
fn b_opens_the_html_part_with_scripts_forbidden() {
    let html = "<!DOCTYPE html><html><body><p>See <i>you</i>.</p></body></html>";
    let page = opened(FakeBody::text("html", "7bit", html), &[Key::Char('B')]);
    assert!(
        page.starts_with("\u{feff}<!DOCTYPE html><meta http-equiv"),
        "{page}"
    );
    assert!(page.contains("script-src 'none'"), "{page}");
    assert!(
        page.ends_with("<p>See <i>you</i>.</p></body></html>"),
        "{page}"
    );
}

// An inline image shows offline; one whose id another's starts with stays
// whole.
#[test]
fn inline_images_are_embedded() {
    let html = "<img src=\"cid:logo\"><img src=\"cid:logo2\">";
    let related = FakeBody::multipart(
        "related",
        vec![
            FakeBody::text("html", "7bit", html),
            FakeBody::file("image", "png").inline("logo"),
            FakeBody::file("image", "gif").inline("logo2"),
        ],
    );
    let page = opened(related, &[Key::Char('B')]);
    assert!(
        page.contains("<img src=\"data:image/png;base64,AAECAwQF\">"),
        "{page}"
    );
    assert!(
        page.contains("<img src=\"data:image/gif;base64,AAECAwQF\">"),
        "{page}"
    );
}

// Plain mail has no markup of its own: its text must not be read as HTML.
#[test]
fn a_text_message_opens_as_escaped_text() {
    let page = opened(
        FakeBody::plain("1 < 2 & <b>not bold</b>"),
        &[Key::Enter, Key::Char('B')],
    );
    assert!(
        page.contains(
            "<pre style=\"white-space: pre-wrap\">1 &lt; 2 &amp; &lt;b&gt;not bold&lt;/b&gt;</pre>"
        ),
        "{page}"
    );
}
