//! The text View of HTML mail, from anonymised `.eml` fixtures built on the
//! patterns of a corpus of real Gmail Messages. `v` switches to it.

#[allow(dead_code, reason = "each test binary uses part of the harness")]
mod support;

use std::time::{Duration, Instant};

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

fn postal(home: &Home, (cols, rows): (u16, u16), no_color: bool) -> Terminal {
    let builder = Terminal::builder()
        .size(cols, rows)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10));
    let builder = if no_color {
        builder.env("NO_COLOR", "1")
    } else {
        builder
    };

    builder.spawn(env!("CARGO_BIN_EXE_postal")).unwrap()
}

fn quit(mut t: Terminal) {
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn fixture(name: &str) -> FakeBody {
    let path = format!("{}/tests/fixtures/{name}.eml", env!("CARGO_MANIFEST_DIR"));
    FakeBody::eml(&std::fs::read(path).unwrap())
}

fn preview(body: FakeBody, size: (u16, u16), shown: &str) -> Screen {
    preview_in(body, size, shown, false)
}

/// The screen once the one Message in the Inbox, `body`, shows `shown` in
/// the text View and sync is done.
fn preview_in(body: FakeBody, size: (u16, u16), shown: &str, no_color: bool) -> Screen {
    let message = FakeMessage::new("Sender", "Subject", "2024-11-18T09:00:00Z").body(body);
    let server = FakeImap::start(vec![message]);
    let home = Home::for_server(&server);
    let mut t = postal(&home, size, no_color);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !server.commands().iter().any(|c| c == "IDLE") {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }

    t.send(Key::Char('v')).unwrap();
    let screen = t.snapshot_after(|s| s.contains(shown)).unwrap();
    quit(t);
    screen
}

const WIDE: (u16, u16) = (120, 40);

// The HTML part wins over its plain twin. Hidden preheaders, CSS-hidden
// blocks, a zero font-size with no size set inside, tracking pixels and
// unlinked images vanish; MJML's zero font-size wrappers keep their text.
#[test]
fn an_mjml_newsletter_shows_only_what_a_mail_client_shows() {
    let screen = preview(fixture("mjml"), WIDE, "release notes");
    insta::assert_snapshot!(screen.with_styles());
}

// A linked image shows its alt; with an empty alt, or none, a link with no
// text shows `[link]`, so image-only mail keeps every link. A link with
// nothing inside shows nothing, as in a browser.
#[test]
fn image_only_mail_keeps_its_links() {
    let screen = preview(fixture("image-only"), WIDE, "[link]");
    insta::assert_snapshot!(screen.with_styles());
}

// A card captions its linked picture with the same title, so the picture
// goes; a picture whose alt says something else stays.
#[test]
fn a_card_shows_its_title_once() {
    let body = FakeBody::text(
        "html",
        "7bit",
        "<table><tr><td><a href=\"https://a.example/1\"><img src=\"c.jpg\" \
         alt=\"AI for App Building &amp; Deployment\"></a></td></tr></table>\n\
         <table><tr><td><a href=\"https://a.example/2\">AI for App Building \
         &amp; Deployment</a></td></tr></table>\n\
         <p><a href=\"https://a.example/3\"><img src=\"d.jpg\" alt=\"Sale\"></a></p>\
         <p>Rust in Action</p>",
    );
    let screen = preview(body, (120, 20), "Rust in Action");
    let text = screen.rect_text(43.., ..);
    assert_eq!(text.matches("AI for App Building").count(), 1, "{text}");
    assert!(text.contains("[Sale]"), "{text}");
}

// Columns stay while every cell fits on one line, and the icon column,
// empty once its images go, takes no room.
#[test]
fn a_data_table_keeps_its_columns_where_they_fit() {
    let screen = preview(fixture("receipt"), WIDE, "orders page");
    insta::assert_snapshot!(screen.with_styles());
}

// Too narrow for its columns, each row stacks as `Header: value` lines,
// and the icon-only and empty cells are dropped.
#[test]
fn a_data_table_too_wide_stacks_its_rows() {
    let screen = preview(fixture("receipt"), (60, 50), "orders page");
    insta::assert_snapshot!(screen.with_styles());
}

#[test]
fn a_gmail_reply_keeps_its_nested_quotes() {
    let screen = preview(fixture("gmail-reply"), WIDE, "next week");
    insta::assert_snapshot!(screen.with_styles());
}

// A windows-1251 part in quoted-printable decodes to readable text.
#[test]
fn cyrillic_in_a_legacy_charset_renders_intact() {
    let screen = preview(fixture("cyrillic"), WIDE, "Север");
    insta::assert_snapshot!(screen.with_styles());
}

#[test]
fn emoji_render_intact() {
    let screen = preview(fixture("emoji"), WIDE, "We are here");
    insta::assert_snapshot!(screen.with_styles());
}

// HTML colours assume a white background: the Preview ignores them and
// keeps only its own semantic styles, which `NO_COLOR` strips too.
#[test]
fn no_color_keeps_only_the_modifiers() {
    let body = FakeBody::text(
        "html",
        "7bit",
        "<p style=\"color:#ff0000\">Red <b>bold</b> <i>italic</i> \
         <a href=\"https://a.example\">link</a> <code>code</code></p>",
    );
    let screen = preview_in(body, (120, 12), "Red bold italic link code", true);
    insta::assert_snapshot!(screen.with_styles());
}

/// About 400 KB of newsletter, as the largest Messages in the corpus.
fn large_newsletter() -> String {
    let article = "<table role=\"presentation\" width=\"100%\"><tr><td style=\"font-size:0\">\
        <div style=\"font-size:16px\"><h2>Story NUMBER</h2><p>Lorem ipsum dolor sit amet, \
        <b>consectetur</b> adipiscing elit, sed do eiusmod tempor incididunt ut labore et \
        dolore magna aliqua. <a href=\"https://news.example/NUMBER\">Read more</a></p>\
        </div></td></tr></table>\n";
    let mut html = String::from("<html><body><p>The very first line</p>\n");
    let mut number = 0;
    while html.len() < 400 * 1024 {
        number += 1;
        html.push_str(&article.replace("NUMBER", &number.to_string()));
    }

    html.push_str("</body></html>");
    html
}

/// Whether the Preview, right of the list at 120 columns, shows `text`.
fn in_preview(screen: &Screen, text: &str) -> bool {
    screen.rect_text(43.., ..).contains(text)
}

// Rendering runs off the UI thread: while the large Message renders, the
// keys still move the selection and draw the other Preview.
#[test]
fn a_large_message_renders_without_blocking_frames() {
    let server = FakeImap::start(vec![
        FakeMessage::new("Sender", "Small", "2024-11-17T09:00:00Z")
            .body(FakeBody::plain("A small body")),
        FakeMessage::new("Sender", "Large", "2024-11-18T09:00:00Z").body(FakeBody::text(
            "html",
            "7bit",
            large_newsletter(),
        )),
    ]);
    let home = Home::for_server(&server);
    let mut t = postal(&home, (120, 30), false);
    t.wait_until(|s| s.contains(" 1/2 ")).unwrap();
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains(" 2/2 ") && in_preview(s, "A small body"))
        .unwrap();
    t.send(Key::Char('k')).unwrap();
    t.wait_until(|s| s.contains(" 1/2 ") && in_preview(s, "The very first line"))
        .unwrap();
    quit(t);
}
