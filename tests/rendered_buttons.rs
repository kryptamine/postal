//! Buttons in the rendered View of HTML mail: one shape, whatever markup
//! made them.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use support::rendered::{DEFAULT, Run, at, bg, fg, frames, html, page, pane, preview};
use termlens::Color;

// litehtml split an inline around a block inside it even through an
// inline-block, which cut a link-wrapped button's box in two empty halves.
#[test]
fn a_link_around_an_inline_block_button_keeps_it_whole() {
    let body = page("a-link-around-an-inline-block-button-keeps-it-whole");
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
    let body = page("a-filled-button-is-one-row-with-no-border");
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
    let body = page("a-cell-in-the-fill-of-its-button-is-cut-back-with-it");
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

// Brands label one button in black and the next in white; white reads the
// same on every fill it reads on at all, and a yellow one keeps its black.
#[test]
fn a_button_is_labelled_in_white_where_white_reads() {
    let button = |fill: &str, label: &str| {
        format!(
            "<p><a href=\"https://shop.example/{label}\" style=\"display:inline-block;\
             background:{fill};color:#000c02;padding:12px 30px\">{label}</a></p>"
        )
    };

    let body = html(&format!(
        "<p>x</p>{}{}",
        button("#00a264", "Browse"),
        button("#ffd500", "Shop")
    ));
    let screen = preview(body, &DEFAULT, "Shop");
    let ink = |label: &str| {
        let (row, col) = at(&screen, label);
        fg(&screen, row, col)
    };

    assert_eq!(
        ink("Browse"),
        Color::Rgb(255, 255, 255),
        "{}",
        screen.text()
    );
    assert_ne!(ink("Shop"), Color::Rgb(255, 255, 255), "{}", screen.text());
}

// Mail draws a button as a fill, a border, a full-width slab or a tight
// box. Each is its label in bold on two cells of fill at either side.
#[test]
fn every_button_has_one_shape() {
    let body = page("every-button-has-one-shape");
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
    let body = page("a-bordered-cell-with-a-link-is-no-button");
    let screen = preview(body, &DEFAULT, "Download");
    let (row, col) = at(&screen, "Download");
    assert_eq!(bg(&screen, row, col), Color::Default, "{}", screen.text());
    assert!(screen.row_text(row - 1).contains('┌'), "{}", screen.text());
}

// A table sized with `height` round a filled button cell stretched the button
// into a three-row slab.
#[test]
fn a_table_height_round_a_button_collapses_with_it() {
    let body = page("a-table-height-round-a-button-collapses-with-it");
    let screen = preview(body, &DEFAULT, "Browse services");
    let (row, col) = at(&screen, "Browse services");
    assert!(matches!(bg(&screen, row, col), Color::Rgb(..)));
    for r in [row - 1, row + 1] {
        assert_eq!(bg(&screen, r, col), Color::Default, "row {r}");
    }
}

// A rounded button ends in half circles where the font has them: a pill,
// as the mail drew it, not a slab.
#[test]
fn a_rounded_button_has_round_ends_in_a_nerd_font() {
    let body = || page("a-rounded-button-has-round-ends-in-a-nerd-font");
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
