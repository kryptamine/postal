//! HTML mail too wide for the pane: what the rendered View lets go of
//! before the text View takes it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use support::rendered::{DEFAULT, at, bg, html, page, pane, preview, quit, started};
use termlens::{Color, Screen};

// A layout that loses text past the pane however it is loosened shows
// in the text View instead.
#[test]
fn a_mail_too_wide_for_the_pane_falls_back_to_text() {
    let body = page("a-mail-too-wide-for-the-pane-falls-back-to-text");
    let (mut t, _server) = started(body, &DEFAULT);
    let shown = |s: &Screen| pane(s).iter().any(|r| r.contains("View run"));
    let screen = t.snapshot_after(shown).unwrap();
    quit(t);
    let (row, col) = at(&screen, "View run");
    assert_eq!(bg(&screen, row, col), Color::Default, "{}", screen.text());
}

// A mail made for 600 px fixes its widths and forbids wrapping, and lost
// its right edge in a narrow pane. Laid out again without them it fits.
#[test]
fn a_mail_wider_than_the_pane_lets_go_of_its_fixed_widths() {
    let body = page("a-mail-wider-than-the-pane-lets-go-of-its-fixed-widths");
    let screen = preview(body, &DEFAULT, "Right column");
    let (row, col) = at(&screen, "Right column");
    assert!(
        screen.row_text(row).contains("Left column"),
        "{}",
        screen.text()
    );
    assert!(matches!(bg(&screen, row, col), Color::Rgb(..)));
}

// Mobile styles fix widths for a phone, and a pane just under their
// breakpoint showed the mail in a column half its width.
#[test]
fn a_mail_set_for_a_phone_takes_its_desktop_layout() {
    let body = page("a-mail-set-for-a-phone-takes-its-desktop-layout");
    let screen = preview(body, &DEFAULT, "phone");
    let (first, last) = (at(&screen, "A line").0, at(&screen, "phone").0);
    assert_eq!(first, last, "{}", screen.text());
}

// An address longer than the pane has no space to wrap at: it breaks after
// its own marks, and every piece of it shows.
#[test]
fn a_word_longer_than_the_pane_breaks() {
    let address = format!("https://shop.example/{}end", "orders/2024/".repeat(8));
    let body = html(&format!(
        "<p>{address}</p><a href=\"https://ci.example\" \
         style=\"background:#1f883d;color:#fff\">View run</a>"
    ));
    let screen = preview(body, &DEFAULT, "View run");
    let (row, col) = at(&screen, "View run");
    assert!(matches!(bg(&screen, row, col), Color::Rgb(..)));
    let shown: String = pane(&screen).iter().map(|r| r.trim()).collect();
    assert!(shown.contains(&address), "{}", screen.text());
}

// Columns that cannot shrink into the pane stack, as a narrow screen shows
// them, where the text View would drop every colour and frame.
#[test]
fn columns_too_wide_for_the_pane_stack() {
    let cell = "<td style=\"background:#1f883d;color:#fff\">Column-number-N</td>";
    let cells: String = ["1", "2", "3", "4", "5", "6"]
        .map(|n| cell.replace('N', n))
        .concat();
    let screen = preview(
        html(&format!("<table><tr>{cells}</tr></table>")),
        &DEFAULT,
        "Column-number-6",
    );
    let (first, _) = at(&screen, "Column-number-1");
    let (last, _) = at(&screen, "Column-number-6");
    assert_eq!(last - first, 5, "{}", screen.text());
    // The fill they share is a section, which the rendered View frames.
    assert!(
        screen.row_text(first - 1).contains('┌'),
        "{}",
        screen.text()
    );
}
