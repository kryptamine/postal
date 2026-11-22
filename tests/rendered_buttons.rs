//! Buttons in the rendered View of HTML mail: one shape, whatever markup
//! made them.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use support::rendered::{DEFAULT, Run, at, bg, frames, html, pane, preview};
use termlens::Color;

// litehtml split an inline around a block inside it even through an
// inline-block, which cut a link-wrapped button's box in two empty halves.
#[test]
fn a_link_around_an_inline_block_button_keeps_it_whole() {
    let body = html(
        "<p>x</p><a href=\"https://shop.example\"><span style=\"background:#1f883d;\
         display:inline-block;padding:16px\"><span style=\"display:block\">Shop now</span>\
         </span></a><p>x</p>",
    );
    let screen = preview(body, &DEFAULT, "Shop now");
    let (row, col) = at(&screen, "Shop now");
    assert!(matches!(bg(&screen, row, col), Color::Rgb(..)));
    for r in [row - 1, row + 1] {
        assert_eq!(bg(&screen, r, col), Color::Default, "{}", screen.text());
    }
}

// A filled button's padding was rows of fill and its border a row more: a
// slab three rows high or a label in a frame the fill was moved against.
#[test]
fn a_filled_button_is_one_row_with_no_border() {
    let body = html(
        "<p>x</p><a href=\"https://ci.example\" style=\"display:inline-block;color:#fff;\
         background:#1f883d;border:1px solid #0b5a20;padding:12px 24px\">View run</a>",
    );
    let screen = preview(body, &DEFAULT, "View run");
    let (row, col) = at(&screen, "View run");
    assert!(
        !pane(&screen).iter().any(|r| r.contains('┌')),
        "{}",
        screen.text()
    );
    assert!(matches!(bg(&screen, row, col), Color::Rgb(..)));
    for r in [row - 1, row + 1] {
        assert_eq!(bg(&screen, r, col), Color::Default, "row {r}");
    }
}

// A cell in the button's own fill, sized for the button, is part of it:
// with its `height: 50px` it stood as a slab of fill round the label.
#[test]
fn a_cell_in_the_fill_of_its_button_is_cut_back_with_it() {
    let body = html(
        "<p>x</p><table width=\"400\"><tr><td align=\"center\" bgcolor=\"#892de1\" \
         style=\"height:50px\"><a href=\"https://learn.example\" style=\"display:inline-block;\
         background:#892de1;color:#fff;padding:10px 40px\">Explore courses</a></td></tr></table>\
         <p>x</p>",
    );
    let screen = preview(body, &DEFAULT, "Explore courses");
    let (row, col) = at(&screen, "Explore courses");
    assert!(matches!(bg(&screen, row, col), Color::Rgb(..)));
    assert_eq!(
        bg(&screen, row, col - 4),
        Color::Default,
        "{}",
        screen.text()
    );
    for r in [row - 1, row + 1] {
        assert_eq!(bg(&screen, r, col), Color::Default, "row {r}");
    }
}

// Mail draws a button as a fill, a border, a full-width slab or a tight
// box. Each is its label in bold on two cells of fill at either side.
#[test]
fn every_button_has_one_shape() {
    let body = html(
        "<p>x</p><a href=\"https://shop.example/a\" style=\"display:block;background:#1f883d;\
         color:#fff;padding:12px 0;text-align:center\">Wide one</a>\
         <p>x</p><div style=\"padding-left:40px\"><a href=\"https://shop.example/b\" \
         style=\"background:#1f883d;color:#fff\">Tight one</a></div>\
         <p>x</p><a href=\"https://shop.example/c\" style=\"display:inline-block;\
         border:1px solid #1a73e8;color:#1a73e8;padding:12px 40px\">Outlined one</a><p>x</p>",
    );
    let screen = preview(body, &DEFAULT, "Outlined one");
    assert_eq!(frames(&screen), 0, "{}", screen.text());
    for label in ["Wide one", "Tight one", "Outlined one"] {
        let (row, col) = at(&screen, label);
        let end = col + u16::try_from(label.len()).unwrap();
        for filled in col - 2..end + 2 {
            assert!(
                matches!(bg(&screen, row, filled), Color::Rgb(..)),
                "{label} at {filled}"
            );
        }

        for bare in [col - 3, end + 2] {
            assert_eq!(bg(&screen, row, bare), Color::Default, "{label} at {bare}");
        }

        assert!(screen.cell(row, col).unwrap().style().bold, "{label}");
        assert_eq!(bg(&screen, row - 1, col), Color::Default, "{label}");
    }
}

// A bordered cell of a table that holds a link is a cell of data: only a
// table of one cell is a button built as a table.
#[test]
fn a_bordered_cell_with_a_link_is_no_button() {
    let body = html(
        "<table><tr><td style=\"border:1px solid #888888\">Invoice 12</td>\
         <td style=\"border:1px solid #888888\"><a href=\"https://pay.example/12\">Download</a>\
         </td></tr></table>",
    );
    let screen = preview(body, &DEFAULT, "Download");
    let (row, col) = at(&screen, "Download");
    assert_eq!(bg(&screen, row, col), Color::Default, "{}", screen.text());
    assert!(screen.row_text(row - 1).contains('┌'), "{}", screen.text());
}

// A rounded button ends in half circles where the font has them: a pill,
// as the mail drew it, not a slab.
#[test]
fn a_rounded_button_has_round_ends_in_a_nerd_font() {
    let body = || {
        html(
            "<p>Text</p><a href=\"https://app.example\" style=\"background:#1a73e8;color:#fff;\
             padding:12px 24px;border-radius:20px;display:inline-block\">Launch App</a><p>End</p>",
        )
    };
    let nerd = Run {
        nerd: true,
        ..DEFAULT
    };

    let screen = preview(body(), &nerd, "Launch App");
    let rows = pane(&screen);
    let row = rows.iter().find(|r| r.contains("Launch App")).unwrap();
    assert!(
        row.contains("\u{e0b6}  Launch App  \u{e0b4}")
            || row.contains("\u{e0b6} Launch App \u{e0b4}"),
        "{row}"
    );
    let plain = preview(body(), &DEFAULT, "Launch App");
    assert!(!plain.contains("\u{e0b6}"), "{}", plain.text());
}
