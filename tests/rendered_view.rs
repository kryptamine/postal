//! The rendered View of HTML mail: its layout on the cell grid, in colours
//! moved onto the terminal's background.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use support::rendered::{DEFAULT, LIGHT, Run, at, bg, fixture, html, pane, preview, quit, started};
use termlens::{Color, Key, Screen};

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
    let screen = preview(fixture("statement"), &DEFAULT, "Never share");
    let rows = pane(&screen);
    let rules = rows.iter().filter(|row| row.contains("────────")).count();
    assert!(rules >= 5, "{}", rows.join("\n"));
    insta::assert_snapshot!(screen.with_styles());
}

// A light terminal keeps the same layout; the near-black button and the
// yellow rule darken or lighten only as far as they must to show.
#[test]
fn a_statement_on_a_light_terminal() {
    let run = Run {
        background: LIGHT,
        ..DEFAULT
    };

    let screen = preview(fixture("statement"), &run, "Never share");
    insta::assert_snapshot!(screen.with_styles());
}

// Without colour the layout, borders and text styles stay.
#[test]
fn no_color_keeps_the_layout_and_drops_the_colours() {
    let run = Run {
        no_color: true,
        ..DEFAULT
    };

    let screen = preview(fixture("statement"), &run, "Never share");
    insta::assert_snapshot!(screen.with_styles());
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

// A border is a row of its own: padding short of a row under it only made a
// box two rows taller.
#[test]
fn padding_under_a_border_adds_no_blank_row() {
    let body = html(
        "<p>x</p><div style=\"display:inline-block;border:1px solid #888;\
         padding:12px 24px\">Run 12</div>",
    );
    let screen = preview(body, &DEFAULT, "Run 12");
    let (row, _) = at(&screen, "Run 12");
    assert!(screen.row_text(row - 1).contains('┌'), "{}", screen.text());
    assert!(screen.row_text(row + 1).contains('└'), "{}", screen.text());
}

// A transparent border never shows; a cell thick it boxed a button's label
// in a frame of the terminal's own colour.
#[test]
fn a_transparent_border_takes_no_room() {
    let body = html(
        "<p>x</p><a href=\"https://ci.example\"><span style=\"display:inline-block;\
         border:1px solid transparent\">Learn more</span></a>",
    );
    let screen = preview(body, &DEFAULT, "Learn more");
    assert!(
        !pane(&screen)
            .iter()
            .any(|r| r.contains('┌') || r.contains('│')),
        "{}",
        screen.text()
    );
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

// Columns of a fluid layout stand on the baseline, which only images of one
// height kept level: without them the cards rose and fell in a zigzag.
#[test]
fn fluid_columns_line_up_at_the_top() {
    let column = |lines: &str| {
        format!(
            "<div style=\"display:inline-block;width:45%\"><table><tr><td>{lines}</td></tr></table></div>"
        )
    };
    let body = html(&format!(
        "<div>{}{}</div>",
        column("Left card"),
        column("Right card<br>second line<br>third line")
    ));
    let screen = preview(body, &DEFAULT, "third line");
    assert_eq!(
        at(&screen, "Left card").0,
        at(&screen, "Right card").0,
        "{}",
        screen.text()
    );
}

// `colspan="0"` is one column in HTML; litehtml took the cell's width from
// the column before the first, outside its grid, and wrapped it word by word.
#[test]
fn a_cell_spanning_no_columns_spans_one() {
    let body = html(
        "<table><tr><td colspan=\"0\">One two</td><td>B</td></tr>\
         <tr><td>One two three</td><td>D</td></tr></table>",
    );
    let screen = preview(body, &DEFAULT, "three");
    assert!(
        pane(&screen)
            .iter()
            .any(|r| r.contains("One two ") && r.contains('B')),
        "{}",
        screen.text()
    );
}

// A mail has no page height to take a share of: `height: 100%` took the
// layout's viewport, and the box stood a thousand rows high.
#[test]
fn a_percentage_height_is_the_height_of_what_it_holds() {
    let body = html(
        "<html style=\"height:100%\"><body style=\"height:100%\">\
         <div style=\"height:100%;min-height:100%;border:1px solid #cc0000\"><p>Inside</p></div>\
         <p>After</p></body></html>",
    );
    let screen = preview(body, &DEFAULT, "After");
    let rows = pane(&screen);
    let row = |text: &str| rows.iter().position(|r| r.contains(text)).unwrap();
    assert_eq!(row("After") - row("Inside"), 3, "{}", rows.join("\n"));
}

// Mail spaces its blocks with rows of padding: one blank row sets two
// blocks apart as well as eight, and a frame's edge needs none inside it.
#[test]
fn a_run_of_blank_rows_is_one() {
    let body = html(
        "<p>Above</p><div style=\"height:128px\"></div>\
         <div style=\"border:1px solid #cc0000;padding:64px 16px\">Inside</div><p>Below</p>",
    );
    let screen = preview(body, &DEFAULT, "Below");
    let rows = pane(&screen);
    let row = |text: &str| rows.iter().position(|r| r.contains(text)).unwrap();
    // A blank row and the frame's edge.
    assert_eq!(row("Inside") - row("Above"), 3, "{}", rows.join("\n"));
    assert_eq!(row("Below") - row("Inside"), 3, "{}", rows.join("\n"));
}
