//! The rendered View of HTML mail: its layout on the cell grid, in colours
//! moved onto the terminal's background.

#[allow(dead_code, reason = "each test binary uses part of the harness")]
mod support;

use std::{
    net::TcpListener,
    time::{Duration, Instant},
};

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

const WIDE: (u16, u16) = (120, 40);
const DARK: (u8, u8, u8) = (0, 0, 0);
const LIGHT: (u8, u8, u8) = (255, 255, 255);

struct Run {
    size: (u16, u16),
    background: (u8, u8, u8),
    no_color: bool,
}

const DEFAULT: Run = Run {
    size: WIDE,
    background: DARK,
    no_color: false,
};

fn fixture(name: &str) -> FakeBody {
    let path = format!("{}/tests/fixtures/{name}.eml", env!("CARGO_MANIFEST_DIR"));
    FakeBody::eml(&std::fs::read(path).unwrap())
}

fn html(content: &str) -> FakeBody {
    FakeBody::text("html", "7bit", content)
}

/// Postal on the one Message `body`, once sync is done.
fn started(body: FakeBody, run: &Run) -> (Terminal, FakeImap) {
    let message = FakeMessage::new("Sender", "Subject", "2024-11-18T09:00:00Z").body(body);
    let server = FakeImap::start(vec![message]);
    let home = Home::for_server(&server);
    let (r, g, b) = run.background;
    let builder = Terminal::builder()
        .size(run.size.0, run.size.1)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .background_rgb(r, g, b)
        .timeout(Duration::from_secs(10));
    let builder = if run.no_color {
        builder.env("NO_COLOR", "1")
    } else {
        builder
    };

    let t = builder.spawn(env!("CARGO_BIN_EXE_postal")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !server.commands().iter().any(|c| c == "IDLE") {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }

    (t, server)
}

fn quit(mut t: Terminal) {
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

/// The screen once the Preview of `body` shows `shown`.
fn preview(body: FakeBody, run: &Run, shown: &str) -> Screen {
    let (mut t, _server) = started(body, run);
    let screen = t.snapshot_after(|s| s.contains(shown)).unwrap();
    quit(t);
    screen
}

/// The Preview's rows right of the list at 120 columns, trailing blanks cut.
fn pane(screen: &Screen) -> Vec<String> {
    screen
        .rect_text(44..118, 4..39)
        .lines()
        .map(|line| line.trim_end().to_owned())
        .collect()
}

// Hidden preheaders and Outlook-only blocks stay hidden, MJML's zero
// font-size wrappers keep their text, and the columns stay side by side.
#[test]
fn an_mjml_newsletter_keeps_its_layout() {
    let screen = preview(fixture("mjml"), &DEFAULT, "release notes");
    insta::assert_snapshot!(screen.with_styles());
}

#[test]
fn a_gmail_reply_draws_a_bar_for_each_quote() {
    let screen = preview(fixture("gmail-reply"), &DEFAULT, "next week");
    insta::assert_snapshot!(screen.with_styles());
}

// Box edges snap to rows: every row separator of the table shows, and the
// button's label sits in the middle row of its block.
#[test]
fn a_statement_keeps_its_table_rules_and_centres_its_button() {
    let screen = preview(fixture("statement"), &DEFAULT, "Bank logo");
    let rows = pane(&screen);
    let rules = rows.iter().filter(|row| row.contains("────────")).count();
    assert!(rules >= 5, "{}", rows.join("\n"));
    insta::assert_snapshot!(screen.with_styles());
}

// A light terminal keeps the same layout; the near-black button and the
// grey text darken or lighten only as far as they must to read.
#[test]
fn a_statement_on_a_light_terminal() {
    let run = Run {
        background: LIGHT,
        ..DEFAULT
    };

    let screen = preview(fixture("statement"), &run, "Bank logo");
    insta::assert_snapshot!(screen.with_styles());
}

// Without colour the layout, borders and text styles stay.
#[test]
fn no_color_keeps_the_layout_and_drops_the_colours() {
    let run = Run {
        no_color: true,
        ..DEFAULT
    };

    let screen = preview(fixture("statement"), &run, "Bank logo");
    insta::assert_snapshot!(screen.with_styles());
}

// A remote image is a tracking risk: nothing is fetched, and its box keeps
// its place so the text under it does not move.
#[test]
fn images_are_never_fetched_and_keep_their_box() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let body = html(&format!(
        "<p>Above</p><img src=\"http://127.0.0.1:{port}/banner.png\" width=\"80\" \
         height=\"48\"><p>Below</p><img src=\"http://127.0.0.1:{port}/logo.png\" alt=\"Logo\">"
    ));
    let screen = preview(body, &DEFAULT, "Logo");
    let rows = pane(&screen);
    let row = |text: &str| rows.iter().position(|r| r.contains(text)).unwrap();
    // 48px is three rows, between the margins of the two paragraphs.
    assert!(row("Below") - row("Above") > 3, "{}", rows.join("\n"));
    assert!(listener.accept().is_err(), "an image was fetched");
}

// `v` switches every HTML Preview to the text View and back.
#[test]
fn v_switches_between_the_rendered_and_the_text_view() {
    let body = html(
        "<table width=\"100%\"><tr><td>Left column</td><td align=\"right\">Right column</td>\
         </tr></table>",
    );
    let (mut t, _server) = started(body, &DEFAULT);
    let side_by_side = |s: &Screen| {
        pane(s)
            .iter()
            .any(|r| r.contains("Left column") && r.contains("Right column"))
    };

    let stacked = |s: &Screen| !side_by_side(s) && s.contains("Right column");
    t.wait_until(side_by_side).unwrap();
    t.send(Key::Char('v')).unwrap();
    t.wait_until(stacked).unwrap();
    t.send(Key::Char('v')).unwrap();
    t.wait_until(side_by_side).unwrap();
    quit(t);
}

// A layout wider than the pane would lose its right edge, so the Preview
// shows the text View instead.
#[test]
fn a_mail_too_wide_for_the_pane_falls_back_to_text() {
    let run = Run {
        size: (42, 40),
        ..DEFAULT
    };

    let screen = preview(fixture("statement"), &run, "Coffee");
    insta::assert_snapshot!(screen.text());
}
