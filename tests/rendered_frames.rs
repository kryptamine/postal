//! Section fills in the rendered View of HTML mail, drawn as frames in
//! their colour.

mod support;

use support::rendered::{DEFAULT, at, bg, fg, frames, html, pane, preview};
use termlens::Color;

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
        "<p>x</p><div style=\"background:#1e50f5;color:#fff;border:1px solid #888;\
         padding:32px\">Boxed</div>",
    );
    let screen = preview(body, &DEFAULT, "Boxed");
    let (row, col) = at(&screen, "Boxed");
    assert_eq!(bg(&screen, row, col), Color::Default);
    assert_eq!(frames(&screen), 1, "{}", screen.text());
}

// A border round the whole mail only boxed the page, a frame round every
// line of it; a band against it still takes it as its shape.
#[test]
fn a_border_round_the_whole_mail_is_not_drawn() {
    let body = html(
        "<div style=\"border:1px solid #888\"><div style=\"background:#1e50f5;color:#fff;\
         padding:16px\">Band</div><p>Body</p></div>",
    );
    let screen = preview(body, &DEFAULT, "Body");
    let (row, col) = at(&screen, "Band");
    assert_eq!(bg(&screen, row, col), Color::Default);
    assert_eq!(frames(&screen), 0, "{}", screen.text());
    assert!(
        !pane(&screen).iter().any(|r| r.contains('│')),
        "{}",
        screen.text()
    );
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

// A light card on a white page gave way to the terminal with the page, and
// the cards of a mail ran together: one set apart from the page is framed.
#[test]
fn a_light_card_on_the_page_gets_a_frame() {
    let body = html(
        "<p>Page</p><div style=\"background:#f0f0ff;padding:32px\">Card</div><p>More page</p>",
    );
    let screen = preview(body, &DEFAULT, "More page");
    let (row, col) = at(&screen, "Card");
    assert_eq!(bg(&screen, row, col), Color::Default);
    assert_eq!(frames(&screen), 1, "{}", screen.text());
}

// The page's colour beside a narrower card is the page, whatever its
// shape: framed or filled, the margins boxed nothing but the card.
#[test]
fn the_page_colour_round_a_card_is_the_page() {
    let lines = "<p>One</p><p>Two</p><p>Three</p><p>Four</p><p>Five</p>";
    let body = html(&format!(
        "<table width=\"100%\" bgcolor=\"#002d74\" cellpadding=\"16\"><tr><td>\
         <div style=\"width:320px;margin:0 auto;background:#fff;color:#222\">{lines}</div>\
         </td></tr></table>\
         <table width=\"100%\" bgcolor=\"#1c2b53\" style=\"color:#fff\"><tr><td>\
         {lines}<p>Footer end</p></td></tr></table>"
    ));
    let screen = preview(body, &DEFAULT, "Footer end");
    let (row, _) = at(&screen, "Three");
    assert_eq!(bg(&screen, row, 44), Color::Default, "{}", screen.text());
    assert_eq!(frames(&screen), 2, "{}", screen.text());
}

// A cover band whose logo and pictures went stood as one line of title in a
// frame eight rows high; one in a colour the mail's cards share is a card.
#[test]
fn a_cover_band_left_with_its_title_is_a_title() {
    let band = |colour: &str, title: &str| {
        format!(
            "<div style=\"background:{colour};color:#fff;padding:24px\">\
             <img src=\"https://cdn.example/logo.png\" width=\"150\" height=\"25\" alt=\"\">\
             <h1 style=\"margin:22px 0 0\">{title}</h1></div>"
        )
    };
    let body = html(&format!(
        "{}<p>Hi</p>{}<p>Between</p>{}<p>End</p>",
        band("#141418", "Last call"),
        band("#1e50f5", "Card one"),
        band("#1e50f5", "Card two"),
    ));
    let screen = preview(body, &DEFAULT, "End");
    let rows = pane(&screen);
    let row = |text: &str| rows.iter().position(|r| r.contains(text)).unwrap();
    assert!(
        !rows[..row("Hi")].iter().any(|r| r.contains('┌')),
        "{}",
        rows.join("\n")
    );
    assert_eq!(frames(&screen), 2, "{}", rows.join("\n"));
}
