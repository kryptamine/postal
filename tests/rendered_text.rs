//! The text of HTML mail in the rendered View: how it is aligned, and the
//! roles its colours are reduced to.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

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

// A list built of table rows, a bullet in one cell and the item in the
// next, took the centring of the cell round it: each item stood half a
// pane from its bullet.
#[test]
fn a_centred_item_beside_a_bullet_cell_is_set_left() {
    let body = html(
        "<table width=\"100%\"><tr><td align=\"center\"><table width=\"100%\"><tr>\
         <td width=\"24\">&bull;</td><td>Antalya</td></tr></table></td></tr></table>",
    );
    let screen = preview(body, &DEFAULT, "Antalya");
    let (row, bullet) = at(&screen, "•");
    let (item_row, item) = at(&screen, "Antalya");
    assert_eq!(row, item_row, "{}", screen.text());
    assert!(item - bullet <= 4, "{}", screen.text());
}

// A floated cell is a block to a browser, which litehtml laid over the
// first glyph of its row: it stays a cell, a column off the one before.
#[test]
fn a_floated_cell_stays_a_cell() {
    let body = html(
        "<table><tr><td style=\"float:left\">Distance</td>\
         <td style=\"float:right\">7 km</td></tr></table>",
    );
    let screen = preview(body, &DEFAULT, "7 km");
    assert!(screen.contains("Distance 7 km"), "{}", screen.text());
}

// The space between two floated tables was drawn over the first letter
// of the first: `Crypto is high risk` read `rypto`.
#[test]
fn a_space_between_two_floats_hides_no_letter() {
    let float = |side: &str, width: u16, text: &str| {
        format!(
            "<table align=\"{side}\" width=\"100%\" style=\"float:{side};width:100%;\
             max-width:{width}px\"><tr><td><p style=\"margin:0\">{text}</p></td></tr></table>"
        )
    };

    let body = html(&format!(
        "<table width=\"600\"><tr><td style=\"padding:8px 35px\">\n{}\n{}\n</td></tr></table>",
        float(
            "left",
            341,
            "Crypto is high risk. Its value can go up or down and you could lose some or \
             all of the money you put in."
        ),
        float("right", 144, "Amsterdam")
    ));
    let screen = preview(body, &DEFAULT, "Amsterdam");
    let rows = pane(&screen);
    assert!(
        rows.iter().any(|r| r.contains("Crypto is high risk")),
        "{}",
        rows.join("\n")
    );
}

// A line is one row: `line-height: 1.5 !important` beat the layout's own
// rule and put a blank row between the lines of a title.
#[test]
fn an_important_line_height_keeps_lines_together() {
    let body = html(
        "<p style=\"width:240px;line-height:1.5 !important\">One two three four five six \
         seven eight nine ten eleven twelve</p><p>Next</p>",
    );
    let screen = preview(body, &DEFAULT, "Next");
    // Three lines, and no blank row between them.
    let (first, last) = (at(&screen, "One").0, at(&screen, "twelve").0);
    assert_eq!(last - first, 2, "{}", screen.text());
}

// Blink sets a table's text left under `<td align=center>`, as a receipt's
// rows read; what a style centres stays centred, as does a block's text.
#[test]
fn a_table_under_an_align_attribute_is_set_left() {
    let body = html(
        "<table width=\"100%\"><tr><td align=\"center\">\
         <table width=\"500\"><tr><td>Total due</td></tr></table>\
         <table width=\"500\"><tr><td style=\"text-align:center\">Centred by its style</td></tr></table>\
         <div>Centred block</div></td></tr></table>",
    );
    let screen = preview(body, &DEFAULT, "Centred block");
    let total = at(&screen, "Total due").1;
    assert!(
        at(&screen, "Centred by its style").1 > total + 10,
        "{}",
        screen.text()
    );
    assert!(
        at(&screen, "Centred block").1 > total + 10,
        "{}",
        screen.text()
    );
}

// `align=center` on a row, or further up, centres the narrow card a cell
// holds, as in Blink: a security alert's card stood at the pane's left.
#[test]
fn a_narrow_table_under_an_align_attribute_stands_centred() {
    let body = html(
        "<table width=\"100%\"><tr><td>Edge</td></tr><tr align=\"center\"><td><div>\
         <table style=\"max-width:240px\"><tr><td>Card</td></tr></table>\
         </div></td></tr></table>",
    );
    let screen = preview(body, &DEFAULT, "Card");
    assert!(
        at(&screen, "Card").1 > at(&screen, "Edge").1 + 20,
        "{}",
        screen.text()
    );
}

// Legal lines set small under the body read as its footer; small text
// above the body's last words is a caption and reads as the body.
#[test]
fn the_small_print_under_the_body_is_muted() {
    let body = html(
        "<p>The first paragraph of the body.</p><p style=\"font-size:12px\">A caption</p>\
         <p>The second paragraph of the body.</p><p>The third paragraph.</p>\
         <p style=\"font-size:12px\">Registered office of the sender</p>",
    );
    let screen = preview(body, &DEFAULT, "Registered");
    assert!(dim(&screen, "Registered office"), "{}", screen.text());
    assert!(!dim(&screen, "A caption"), "{}", screen.text());
    assert!(!dim(&screen, "The third"), "{}", screen.text());
}

// Mobile styles reorder stacked cells as table groups; litehtml drew the
// words of such a cell over each other, and a section of the mail was lost.
#[test]
fn a_cell_shown_as_a_table_group_keeps_its_words() {
    let body = html(
        "<style>.last{display:table-footer-group !important}</style>\
         <table width=\"100%\"><tr><td class=\"last\"><h6>Supercharge your skills</h6>\
         <p>Achieve your goals with tailored trainings.</p></td>\
         <td width=\"155\">Side</td></tr></table>",
    );
    let screen = preview(body, &DEFAULT, "Side");
    let rows = pane(&screen);
    for words in [
        "Supercharge your skills",
        "Achieve your goals with tailored trainings.",
    ] {
        assert!(
            rows.iter().any(|r| r.contains(words)),
            "{}",
            rows.join("\n")
        );
    }
}

// Two cells of words side by side are set apart: in 600 px the table
// spread them, in the pane `Support` met `Unsubscribe`.
#[test]
fn the_words_of_two_cells_are_set_apart() {
    let body = html("<table><tr><td>Support</td><td>Unsubscribe</td></tr></table>");
    let screen = preview(body, &DEFAULT, "Unsubscribe");
    assert!(screen.contains("Support Unsubscribe"), "{}", screen.text());
}

// A spacer cell sets two tables apart by its height alone: litehtml read
// only the row's own, and the lines of a receipt ran together.
#[test]
fn a_spacer_cell_keeps_its_row() {
    let body = html(
        "<table><tr><td>Total</td></tr></table>\
         <table width=\"100%\"><tr><td height=\"32\" style=\"max-height:1px\"></td></tr></table>\
         <table><tr><td>Paid</td></tr></table>",
    );
    let screen = preview(body, &DEFAULT, "Paid");
    let (total, paid) = (at(&screen, "Total").0, at(&screen, "Paid").0);
    assert_eq!(paid - total, 2, "{}", screen.text());
}

// Cents set in `<sup>` read as part of the price: `5846 €` for 58.46.
#[test]
fn the_digits_of_a_sup_are_raised() {
    let body = html(
        "<p>Price 58<sup>46</sup> €</p>\
         <table cellspacing=\"0\" cellpadding=\"0\"><tr><td>14</td><td><sup>17</sup></td>\
         <td>€</td></tr></table>",
    );
    let screen = preview(body, &DEFAULT, "Price");
    assert!(screen.contains("Price 58⁴⁶ €"), "{}", screen.text());
    assert!(screen.contains("14¹⁷ €"), "{}", screen.text());
}

// Every glyph is one cell, so a title differs from the body only by its
// style: a heading the mail set in no colour of its own takes Postal's.
#[test]
fn a_heading_takes_the_title_style() {
    let body =
        html("<h1 style=\"font-size:32px\">Your market recap</h1><p>Body text of the mail.</p>");
    let screen = preview(body, &DEFAULT, "Body text");
    let (row, col) = at(&screen, "Your market");
    assert_eq!(
        fg(&screen, row, col),
        Color::Indexed(4),
        "{}",
        screen.text()
    );
    let (row, col) = at(&screen, "Body text");
    assert_eq!(fg(&screen, row, col), Color::Default, "{}", screen.text());
}
