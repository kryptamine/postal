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
use termlens::{Color, Key, Screen, Terminal};

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

// A remote image is a tracking risk: nothing is fetched. One with nothing to
// say takes no room, so a banner leaves no blank rows behind.
#[test]
fn images_are_never_fetched_and_one_without_alt_takes_no_room() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let body = html(&format!(
        "<p>Above</p><img src=\"http://127.0.0.1:{port}/banner.png\" width=\"600\" \
         height=\"300\"><p>Below</p><img src=\"http://127.0.0.1:{port}/logo.png\" alt=\"Logo\">"
    ));
    let screen = preview(body, &DEFAULT, "Logo");
    let rows = pane(&screen);
    let row = |text: &str| rows.iter().position(|r| r.contains(text)).unwrap();
    // One blank row: the margins of the two paragraphs.
    assert_eq!(row("Below") - row("Above"), 2, "{}", rows.join("\n"));
    assert!(listener.accept().is_err(), "an image was fetched");
}

// An alt is one row, however tall the image, and no wider than it, so the
// layout around it keeps its shape.
#[test]
fn an_alt_is_one_row_and_no_wider_than_its_image() {
    let body = html(
        "<p>Above</p><img src=\"https://cdn.example/banner.png\" width=\"80\" height=\"96\" \
         alt=\"Spring collection banner\"><p>Below</p>",
    );
    let screen = preview(body, &DEFAULT, "Spring");
    let rows = pane(&screen);
    let row = |text: &str| rows.iter().position(|r| r.contains(text)).unwrap();
    assert_eq!(row("Below") - row("Above"), 4, "{}", rows.join("\n"));
    // 80px is ten columns.
    assert_eq!(rows[row("Spring")].trim(), "Spring co…");
}

// Mail reuses one image file with a different alt each time; each keeps
// its own.
#[test]
fn images_from_one_file_keep_their_own_alts() {
    let body = html(
        "<img src=\"https://cdn.example/card.png\" width=\"200\" alt=\"First card\"><br>\
         <img src=\"https://cdn.example/card.png\" width=\"200\" alt=\"Second card\">",
    );
    let screen = preview(body, &DEFAULT, "Second card");
    assert!(screen.contains("First card"), "{}", screen.text());
}

// Icons and alts that repeat their row's text are what doubled words in a
// CI report's table: `macOS macOS`, a cut `an` beside each count.
#[test]
fn an_icon_or_an_alt_beside_the_same_text_is_dropped() {
    let body = html(
        "<table><tr>\
         <td><img src=\"https://cdn.example/ok.png\" width=\"24\" height=\"24\" alt=\"macOS\"></td>\
         <td>CI / macOS</td>\
         <td><a href=\"https://ci.example/notes\"><img src=\"https://cdn.example/note.png\" \
         width=\"16\" height=\"16\" alt=\"annotations for CI / macOS\"> 3</a></td>\
         </tr></table>\
         <div><img src=\"https://cdn.example/jane.png\" width=\"84\" height=\"84\" alt=\"Jane Doe\">\
         <div>Jane Doe, product manager</div></div>",
    );
    let screen = preview(body, &DEFAULT, "product manager");
    let rows = pane(&screen);
    let row = rows.iter().find(|r| r.contains("CI / macOS")).unwrap();
    assert_eq!(
        row.split_whitespace().collect::<Vec<_>>(),
        ["CI", "/", "macOS", "3"]
    );
    let names = rows.iter().filter(|r| r.contains("Jane Doe")).count();
    assert_eq!(names, 1, "{}", rows.join("\n"));
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

// A rule is one line: four borders a cell thick drew an empty box instead.
#[test]
fn a_rule_is_one_line() {
    let screen = preview(html("<p>Above</p><hr><p>Below</p>"), &DEFAULT, "Below");
    let rows = pane(&screen);
    let row = |text: &str| rows.iter().position(|r| r.contains(text)).unwrap();
    assert_eq!(row("Below") - row("Above"), 4, "{}", rows.join("\n"));
    assert!(
        rows[row("Above") + 2].contains("────"),
        "{}",
        rows.join("\n")
    );
    assert!(!rows.iter().any(|r| r.contains('┌')), "{}", rows.join("\n"));
}

/// The screen row and column where `text` starts, in the Preview pane.
fn at(screen: &Screen, text: &str) -> (u16, u16) {
    (4..39)
        .find_map(|row| {
            let line: Vec<char> = screen.row_text(row).chars().collect();
            let needle: Vec<char> = text.chars().collect();
            let col = (44..line.len()).find(|&c| line[c..].starts_with(&needle))?;
            Some((row, u16::try_from(col).unwrap()))
        })
        .unwrap_or_else(|| panic!("no {text:?} in\n{}", screen.text()))
}

fn bg(screen: &Screen, row: u16, col: u16) -> Color {
    screen.cell(row, col).unwrap().style().bg
}

// litehtml split an inline around a block inside it even through an
// inline-block, which cut a link-wrapped button's box in two empty halves.
#[test]
fn a_link_around_an_inline_block_button_keeps_it_whole() {
    let body = html(
        "<p>x</p><a href=\"https://shop.example\"><span style=\"border:2px solid #888;\
         display:inline-block\"><span style=\"display:block\">Shop now</span></span></a>",
    );
    let screen = preview(body, &DEFAULT, "Shop now");
    let (row, _) = at(&screen, "Shop now");
    assert!(screen.row_text(row - 1).contains('┌'), "{}", screen.text());
    assert!(screen.row_text(row).contains('│'), "{}", screen.text());
    assert!(screen.row_text(row + 1).contains('└'), "{}", screen.text());
}

// A border is a row of its own: padding short of a row under it only made a
// button two rows taller.
#[test]
fn padding_under_a_border_adds_no_blank_row() {
    let body = html(
        "<p>x</p><a href=\"https://ci.example\" style=\"display:inline-block;\
         border:1px solid #888;padding:12px 24px\">View run</a>",
    );
    let screen = preview(body, &DEFAULT, "View run");
    let (row, _) = at(&screen, "View run");
    assert!(screen.row_text(row - 1).contains('┌'), "{}", screen.text());
    assert!(screen.row_text(row + 1).contains('└'), "{}", screen.text());
}

// A border the colour of its button's fill does not show in a browser; drawn
// a cell thick it framed the label in a line the fill had to be moved from.
#[test]
fn a_border_the_colour_of_its_fill_is_not_drawn() {
    let body = html(
        "<p>x</p><a href=\"https://ci.example\" style=\"display:inline-block;color:#fff;\
         background:#1f883d;border:1px solid #1f883d;padding:12px 24px\">View run</a>",
    );
    let screen = preview(body, &DEFAULT, "View run");
    let (row, col) = at(&screen, "View run");
    assert!(
        !pane(&screen).iter().any(|r| r.contains('┌')),
        "{}",
        screen.text()
    );
    for r in [row - 1, row, row + 1] {
        assert!(matches!(bg(&screen, r, col), Color::Rgb(..)), "row {r}");
    }
}

// A border a cell thick is drawn round a box's background, as a 1px one
// looks, not on top of it.
#[test]
fn a_box_background_stays_inside_its_border() {
    let body = html(
        "<p>x</p><div style=\"width:200px;color:#fff;background:#1f883d;\
         border:1px solid #888\">Inside</div>",
    );
    let screen = preview(body, &DEFAULT, "Inside");
    let (row, col) = at(&screen, "Inside");
    assert!(matches!(bg(&screen, row, col), Color::Rgb(..)));
    assert_eq!(
        bg(&screen, row - 1, col),
        Color::Default,
        "{}",
        screen.text()
    );
}

fn fg(screen: &Screen, row: u16, col: u16) -> Color {
    screen.cell(row, col).unwrap().style().fg
}

/// How many frames the Preview draws, by their top left corners.
fn frames(screen: &Screen) -> usize {
    pane(screen).iter().map(|r| r.matches('┌').count()).sum()
}

// A slab of the mail's colour had its text moved against it, differently on
// a light and a dark terminal: a section shows as a frame in its colour, with
// its text on the terminal's own background. A small control keeps its fill.
#[test]
fn a_section_fill_becomes_a_frame_in_its_colour() {
    let body = html(
        "<div style=\"background:#1e50f5;color:#fff;padding:32px\">Banner</div>\
         <p><span style=\"background:#000;color:#fff\">Go</span></p>",
    );
    let screen = preview(body, &DEFAULT, "Go");
    let (row, col) = at(&screen, "Banner");
    assert_eq!(bg(&screen, row, col), Color::Default);
    let corner = screen.row_text(row - 2).chars().position(|c| c == '┌');
    let corner = u16::try_from(corner.expect("a frame")).unwrap();
    assert!(matches!(fg(&screen, row - 2, corner), Color::Rgb(..)));
    assert!(screen.row_text(row + 2).contains('└'), "{}", screen.text());
    let (row, col) = at(&screen, "Go");
    assert!(matches!(bg(&screen, row, col), Color::Rgb(..)));
}

// A frame drawn through a section's first and last lines would cut their
// words: with text on its edge, the edge moves out into the blank row there.
#[test]
fn a_frame_steps_off_text_on_the_edge_of_its_section() {
    let body = html(
        "<p>Above</p><div style=\"background:#1e50f5;color:#fff\">First line<br>Second line\
         <br>Third line</div><p>Below</p>",
    );
    let screen = preview(body, &DEFAULT, "Below");
    let (row, _) = at(&screen, "First line");
    let corner = screen.row_text(row - 1).chars().position(|c| c == '┌');
    let corner = u16::try_from(corner.expect("a frame")).unwrap();
    // The moved edge keeps the fill's colour, on cells that never had it.
    assert!(matches!(fg(&screen, row - 1, corner), Color::Rgb(..)));
    assert!(screen.row_text(row + 3).contains('└'), "{}", screen.text());
    for line in ["First line", "Second line", "Third line"] {
        assert!(screen.contains(line), "{}", screen.text());
    }
}

// Mail builds one coloured block out of many cells of the same colour; a
// frame for each drew a lattice.
#[test]
fn sections_of_one_colour_that_touch_share_a_frame() {
    let cell = "<td bgcolor=\"#1e50f5\" style=\"color:#fff;padding:32px\">";
    let body = html(&format!(
        "<table width=\"80%\" cellspacing=\"0\" cellpadding=\"0\"><tr>{cell}Left</td>\
         {cell}Right</td></tr><tr>{cell}Under</td>{cell}Last</td></tr></table>"
    ));
    let screen = preview(body, &DEFAULT, "Last");
    assert_eq!(frames(&screen), 1, "{}", screen.text());
}

// The mail's own border already gives the section its shape.
#[test]
fn a_section_with_a_border_of_its_own_gets_no_second_frame() {
    let body = html(
        "<div style=\"background:#1e50f5;color:#fff;border:1px solid #888;padding:32px\">\
         Boxed</div>",
    );
    let screen = preview(body, &DEFAULT, "Boxed");
    let (row, col) = at(&screen, "Boxed");
    assert_eq!(bg(&screen, row, col), Color::Default);
    assert_eq!(frames(&screen), 1, "{}", screen.text());
}

// A dark mail is dark all over: its page gives way to the terminal like a
// white one, and a frame round the whole Preview would say nothing.
#[test]
fn a_dark_page_goes_without_a_frame() {
    let body = html(
        "<body style=\"background:#101418;color:#eee\"><p>One</p><p>Two</p><p>Three</p>\
         <p>Four</p></body>",
    );
    let screen = preview(body, &DEFAULT, "Four");
    let (row, col) = at(&screen, "One");
    assert_eq!(bg(&screen, row, col), Color::Default);
    assert_eq!(frames(&screen), 0, "{}", screen.text());
}

// The bars of a chart are fills of one colour that touch: a frame round
// their outline drew a staircase with the labels caught in it.
#[test]
fn fills_that_make_no_rectangle_stay_fills() {
    let bar = "background:#1e50f5;color:#fff;height:64px";
    let body = html(&format!(
        "<div style=\"{bar};width:400px\">Long bar</div>\
         <div style=\"{bar};width:200px\">Short bar</div>"
    ));
    let screen = preview(body, &DEFAULT, "Short bar");
    for text in ["Long bar", "Short bar"] {
        let (row, col) = at(&screen, text);
        assert!(matches!(bg(&screen, row, col), Color::Rgb(..)), "{text}");
    }

    assert_eq!(frames(&screen), 0, "{}", screen.text());
}

// An icon that is all its link shows is named by its alt, which cut names
// nothing; where its cell is too narrow, it is cut to fit there.
#[test]
fn a_linked_icon_shows_its_whole_alt_where_it_fits() {
    let icon = "<a href=\"https://social.example\"><img src=\"https://cdn.example/i.png\" \
                width=\"24\" height=\"24\" alt=\"Instagram\"></a>";
    let body = html(&format!(
        "<table style=\"width:40px\"><tr><td style=\"width:40px;max-width:40px;\
         overflow:hidden\">{icon}</td></tr></table><p>Free</p>{icon}"
    ));
    let screen = preview(body, &DEFAULT, "Free");
    let rows = pane(&screen);
    let named: Vec<_> = rows.iter().filter(|r| r.contains("Ins")).collect();
    assert_eq!(named.len(), 2, "{}", rows.join("\n"));
    assert!(named[0].contains('…') && !named[0].contains("Instagram"));
    assert!(named[1].contains("Instagram"));
}
