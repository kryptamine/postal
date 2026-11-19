//! The text of HTML mail in the rendered View: how it is aligned, and the
//! roles its colours are reduced to.

mod support;

use support::rendered::{DEFAULT, at, bg, dim, fg, html, pane, preview};
use termlens::Color;

// Justified, one-cell glyphs are padded with whole cells, which tears
// holes in a narrow column.
#[test]
fn justified_text_is_set_left() {
    let body = html(
        "<div style=\"width:200px;text-align:justify\">Order a party of food for three \
         weeks at a closed sale price today</div>",
    );
    let screen = preview(body, &DEFAULT, "today");
    let rows = pane(&screen);
    assert!(
        !rows.iter().any(|r| r.trim().contains("  ")),
        "{}",
        rows.join("\n")
    );
}

// A centred paragraph starts every line somewhere else. Three wrapped
// lines are one; a title of two stays centred, and so do lines the mail
// broke itself.
#[test]
fn a_centred_paragraph_is_set_left() {
    let words = "A long paragraph of the mail that wraps onto three lines of the pane \
                 at the least, since it holds a good deal more words than one row of \
                 seventy-four cells can ever take in, and ends here.";
    let body = html(&format!(
        "<div style=\"text-align:center\"><h1>Short title</h1><p>{words}</p>\
         <p>First<br>Second<br>Third</p></div>"
    ));
    let screen = preview(body, &DEFAULT, "ends here");
    let rows = pane(&screen);
    let indent = |text: &str| {
        let row = rows.iter().find(|r| r.contains(text)).unwrap();
        row.len() - row.trim_start().len()
    };

    assert!(indent("Short title") > 20, "{}", rows.join("\n"));
    assert!(indent("Second") > 20, "{}", rows.join("\n"));
    assert_eq!(indent("A long paragraph"), indent("ends here"));
}

// Centred, a list item's words stand far from the marker that names them
// an item.
#[test]
fn a_centred_list_item_is_set_left() {
    let body = html("<ul style=\"text-align:center\"><li>Vote for ideas</li></ul>");
    let screen = preview(body, &DEFAULT, "Vote for ideas");
    let rows = pane(&screen);
    assert!(
        rows.iter().any(|r| r.contains("• Vote for ideas")),
        "{}",
        rows.join("\n")
    );
}

// The footer is the same in every mail and is read in none: from the block
// with the unsubscribe link on it is muted, its colours and fills gone,
// and the body above keeps its own.
#[test]
fn the_footer_is_muted() {
    let body = html(
        "<p style=\"color:#cc0000\">Your order has shipped.</p>\
         <p>Track it from your account.</p>\
         <p style=\"color:#cc0000\">Acme Ltd, 1 Main Street<br>\
         <a href=\"https://mail.example/unsub?u=1\" style=\"background:#1f883d;color:#fff\">\
         Stop these emails</a></p><p style=\"color:#cc0000\">Sent by a robot.</p>",
    );
    let screen = preview(body, &DEFAULT, "Sent by a robot");
    assert!(!dim(&screen, "Your order"), "{}", screen.with_styles());
    for text in ["Acme Ltd", "Stop these emails", "Sent by a robot"] {
        let (row, col) = at(&screen, text);
        assert!(dim(&screen, text), "{text}\n{}", screen.with_styles());
        assert_eq!(fg(&screen, row, col), Color::Default, "{text}");
        assert_eq!(bg(&screen, row, col), Color::Default, "{text}");
    }
}

// A copyright line far above the end is part of what the mail says.
#[test]
fn a_copyright_line_in_the_body_is_not_the_footer() {
    let lines = "<p>More of the body.</p>".repeat(20);
    let body = html(&format!(
        "<p>© 2024 Acme, quoted here.</p>{lines}<p>The end.</p>"
    ));
    let screen = preview(body, &DEFAULT, "© 2024 Acme");
    assert!(!dim(&screen, "© 2024 Acme"), "{}", screen.with_styles());
}

// Mail sets its secondary text in any grey it likes; here each is the one
// muted style, and the mail's ordinary text is the terminal's own.
#[test]
fn text_greyer_than_the_ordinary_is_muted() {
    let body = html(
        "<p style=\"color:#222222\">Receipt for the month of May, paid in full.</p>\
         <p style=\"color:#8a8a8a\">Invoice number</p><p style=\"color:#999999\">Paid on</p>",
    );
    let screen = preview(body, &DEFAULT, "Paid on");
    let (row, col) = at(&screen, "Receipt");
    assert_eq!(fg(&screen, row, col), Color::Default);
    assert!(!dim(&screen, "Receipt"));
    for text in ["Invoice number", "Paid on"] {
        let (row, col) = at(&screen, text);
        assert!(dim(&screen, text), "{}", screen.with_styles());
        assert_eq!(fg(&screen, row, col), Color::Default);
    }
}

// A mail set whole in a mid grey means it as its ordinary text: muted, the
// whole body would be hard to read.
#[test]
fn a_mail_set_in_grey_is_not_muted() {
    let body = html(
        "<div style=\"color:#666666\"><p>The whole body of the mail is set in one grey.</p>\
         <p style=\"color:#aaaaaa\">A lighter note</p></div>",
    );
    let screen = preview(body, &DEFAULT, "A lighter note");
    assert!(!dim(&screen, "The whole body"), "{}", screen.with_styles());
    assert!(dim(&screen, "A lighter note"), "{}", screen.with_styles());
}

// A mail's links, titles and prices come in shades of its one colour, and
// each drew in its own. A colour of another hue means something else.
#[test]
fn shades_of_one_colour_are_one_colour() {
    let body = html(
        "<p style=\"color:#1a73e8\">A title in the brand blue, the most used shade</p>\
         <p style=\"color:#4285f4\">A lighter blue link</p>\
         <p style=\"color:#d93025\">An overdue amount in red</p>",
    );
    let screen = preview(body, &DEFAULT, "overdue");
    let colour = |text: &str| {
        let (row, col) = at(&screen, text);
        fg(&screen, row, col)
    };

    assert!(matches!(colour("A title"), Color::Rgb(..)));
    assert_eq!(colour("A lighter blue"), colour("A title"));
    assert_ne!(colour("An overdue"), colour("A title"));
}
