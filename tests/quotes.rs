//! A Reply is shown without the quote it ends in: one line stands for it,
//! and `z` shows it again.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::time::{Duration, Instant};

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
    quit,
    rendered::page,
};
use termlens::{Key, Screen, Terminal, TerminalBuilder};

/// The Message List's columns beside the Preview at 120 columns.
const LIST: usize = 42;

/// The line that stands for a quote left out.
const CUT: &str = "… quoted text hidden (z)";

const PLAIN: &str = "Sounds good\r\n\r\n\
    On Mon, 4 Mar 2024 at 09:00, Bob <bob@example.com> wrote:\r\n\
    > Where to?\r\n";

fn terminal(home: &Home) -> TerminalBuilder {
    home.terminal().size(120, 30)
}

/// A Message from Ada that answers another.
fn reply(body: FakeBody) -> FakeMessage {
    FakeMessage::new("Ada", "Re: Plans", "2024-03-05T09:00:00Z")
        .body(body)
        .header("In-Reply-To", "<plans@example.com>")
}

/// The Preview pane's text: the Snippet in the list holds the quote too.
fn preview(screen: &Screen) -> String {
    (0..screen.rows())
        .map(|row| screen.row_text(row).chars().skip(LIST).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Postal on the one `message`, with its body in the Preview.
fn shown(message: FakeMessage) -> (Terminal, FakeImap) {
    let server = FakeImap::start(vec![message]);
    let home = Home::for_server(&server);
    let mut t = terminal(&home).spawn(env!("CARGO_BIN_EXE_postal")).unwrap();
    t.wait_until(|s| preview(s).contains("From    Ada"))
        .unwrap();
    (t, server)
}

#[test]
fn a_reply_ends_where_its_quote_starts_and_z_shows_it() {
    let (mut t, _server) = shown(reply(FakeBody::plain(PLAIN)));
    let screen = t.snapshot_after(|s| preview(s).contains(CUT)).unwrap();
    let pane = preview(&screen);
    assert!(pane.contains("Sounds good"), "{screen}");
    // The line naming whom the quote is of goes with it.
    assert!(!pane.contains("wrote:"), "{screen}");
    assert!(!pane.contains("Where to?"), "{screen}");

    t.send(Key::Char('z')).unwrap();
    let screen = t
        .snapshot_after(|s| preview(s).contains("> Where to?"))
        .unwrap();
    assert!(preview(&screen).contains("wrote:"), "{screen}");
    assert!(!preview(&screen).contains(CUT), "{screen}");

    t.send(Key::Char('z')).unwrap();
    t.wait_until(|s| preview(s).contains(CUT) && !preview(s).contains("Where to?"))
        .unwrap();
    quit(t);
}

// The quote is cut from the HTML itself, so the rendered and the text
// View lose it alike.
#[test]
fn an_html_reply_loses_its_blockquote_in_both_views() {
    let (mut t, _server) = shown(reply(page("gmail-quote")));
    for _ in 0..2 {
        let screen = t.snapshot_after(|s| preview(s).contains(CUT)).unwrap();
        let pane = preview(&screen);
        assert!(pane.contains("Sounds good"), "{screen}");
        assert!(!pane.contains("wrote:"), "{screen}");
        assert!(!pane.contains("Where to?"), "{screen}");

        t.send(Key::Char('z')).unwrap();
        t.wait_until(|s| preview(s).contains("Where to?") && preview(s).contains("wrote:"))
            .unwrap();
        t.send(Key::Char('z')).unwrap();
        t.wait_until(|s| preview(s).contains(CUT)).unwrap();
        t.send(Key::Char('v')).unwrap();
        t.wait_until(|s| preview(s).contains("Sounds good"))
            .unwrap();
    }

    quit(t);
}

// A forward or a digest quotes mail the reader has not seen: nothing else
// in the Thread would show it.
#[test]
fn a_message_that_answers_nothing_keeps_its_quote() {
    let lone = FakeMessage::new("Ada", "Fwd: Plans", "2024-03-05T09:00:00Z");
    let (mut t, _server) = shown(lone.body(FakeBody::plain(PLAIN)));
    let screen = t
        .snapshot_after(|s| preview(s).contains("> Where to?"))
        .unwrap();
    assert!(!preview(&screen).contains(CUT), "{screen}");
    quit(t);
}

// An answer written under the quote would make no sense without it.
#[test]
fn a_quote_with_words_after_it_stays() {
    let plain = "> Where to?\r\nThe coast\r\n";
    let (mut t, _server) = shown(reply(FakeBody::plain(plain)));
    let screen = t
        .snapshot_after(|s| preview(s).contains("The coast"))
        .unwrap();
    assert!(preview(&screen).contains("> Where to?"), "{screen}");
    assert!(!preview(&screen).contains(CUT), "{screen}");
    quit(t);

    let (mut t, _server) = shown(reply(page("inline-quote")));
    let screen = t
        .snapshot_after(|s| preview(s).contains("The coast"))
        .unwrap();
    assert!(preview(&screen).contains("Where to?"), "{screen}");
    assert!(!preview(&screen).contains(CUT), "{screen}");
    quit(t);
}

// Only a line with a date or an address names whom a quote is of: the
// Reply's own last words may end in a colon too.
#[test]
fn the_reply_s_own_line_above_the_quote_stays() {
    let plain = "This is the part I mean:\r\n> Where to?\r\n";
    let (mut t, _server) = shown(reply(FakeBody::plain(plain)));
    let screen = t.snapshot_after(|s| preview(s).contains(CUT)).unwrap();
    assert!(preview(&screen).contains("the part I mean:"), "{screen}");
    assert!(!preview(&screen).contains("Where to?"), "{screen}");
    quit(t);
}

// A mail client breaks a long line naming whom the quote is of, and its
// first half alone looks like the Message's own words.
#[test]
fn a_line_above_the_quote_wrapped_in_two_goes_whole() {
    let plain = "Sounds good\r\n\r\n\
        On Monday, 4 March 2024 at 09:00, Bartholomew Montgomery <\r\n\
        bartholomew.montgomery@example.com> wrote:\r\n\
        > Where to?\r\n";
    let (mut t, _server) = shown(reply(FakeBody::plain(plain)));
    let screen = t.snapshot_after(|s| preview(s).contains(CUT)).unwrap();
    assert!(preview(&screen).contains("Sounds good"), "{screen}");
    assert!(!preview(&screen).contains("Bartholomew"), "{screen}");
    quit(t);
}

// Outlook wraps the quoted mail in no element and names what it answers
// in `References` alone: its header block is where the quote starts.
#[test]
fn an_outlook_reply_ends_at_the_header_of_the_quoted_mail() {
    let message = FakeMessage::new("Ada", "RE: Plans", "2024-03-05T09:00:00Z")
        .body(page("outlook-reply"))
        .header("References", "<plans@example.com>");
    let (mut t, _server) = shown(message);
    let screen = t.snapshot_after(|s| preview(s).contains(CUT)).unwrap();
    let pane = preview(&screen);
    assert!(pane.contains("Agreed"), "{screen}");
    assert!(!pane.contains("Sent:"), "{screen}");
    assert!(!pane.contains("Where to?"), "{screen}");
    quit(t);
}

// In Reading `z` is the cursor's Message alone: the other unfolded Reply
// keeps its quote hidden.
#[test]
fn in_reading_z_shows_the_quote_of_the_message_at_the_cursor() {
    let quoting = |sender: &'static str, day: u32, quote: &str| {
        let date = format!("2024-03-{day:02}T09:00:00Z");
        let body = format!("{sender} agrees\r\n\r\n> {quote}\r\n");
        FakeMessage::new(sender, "Re: Plans", &date)
            .body(FakeBody::plain(&body))
            .header("In-Reply-To", "<plans@example.com>")
            .in_thread(0)
            .unread()
    };

    let server = FakeImap::start(vec![
        quoting("Ada", 1, "First quote"),
        quoting("Bob", 2, "Second quote"),
    ]);
    let home = Home::for_server(&server);
    let mut t = terminal(&home).spawn(env!("CARGO_BIN_EXE_postal")).unwrap();
    t.wait_until(|s| s.contains("Bob agrees")).unwrap();
    t.send(Key::Enter).unwrap();
    let cuts = |s: &Screen| s.text().matches(CUT).count();
    // The cursor starts on Ada's, the oldest Unread.
    t.wait_until(|s| s.contains("│▎Ada agrees") && cuts(s) == 2)
        .unwrap();

    t.send(Key::Char('z')).unwrap();
    let screen = t.snapshot_after(|s| s.contains("> First quote")).unwrap();
    assert_eq!(cuts(&screen), 1, "{screen}");
    assert!(!screen.contains("Second quote"), "{screen}");
    quit(t);
}

// The pager is where a Message is read whole.
#[test]
fn the_pager_shows_the_quote() {
    let server = FakeImap::start(vec![reply(FakeBody::plain(PLAIN))]);
    let home = Home::for_server(&server);
    let dir = tempfile::tempdir().unwrap();
    let record = dir.path().join("paged");
    let command = format!("cat > '{}'", record.display());
    let mut t = terminal(&home)
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("PAGER", &command)
        .env("TMPDIR", dir.path())
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    t.wait_until(|s| preview(s).contains(CUT)).unwrap();
    t.send(Key::Char('|')).unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    let paged = loop {
        let paged = std::fs::read_to_string(&record).unwrap_or_default();
        if paged.contains("Sounds good") {
            break paged;
        }

        assert!(Instant::now() < deadline, "timed out: {paged}");
        std::thread::sleep(Duration::from_millis(10));
    };

    // A Ctrl-c before the TUI takes the screen back interrupts it.
    t.wait_until(|s| s.alternate_screen() && preview(s).contains(CUT))
        .unwrap();
    quit(t);
    assert!(paged.contains("> Where to?"), "{paged}");
    assert!(!paged.contains(CUT), "{paged}");
}
