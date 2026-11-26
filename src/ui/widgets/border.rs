use ratatui::{buffer::Buffer, layout::Rect, style::Style, symbols::line};
use unicode_width::UnicodeWidthStr;

use crate::ui::widgets::truncate;

/// `├───┤` across the bordered `pane` on row `y`, joined to its sides, over
/// whatever was drawn there.
pub fn rule(buf: &mut Buffer, pane: Rect, y: u16, style: Style) {
    let right = pane.right().saturating_sub(1);
    for x in pane.x..=right {
        let symbol = if x == pane.x {
            line::VERTICAL_RIGHT
        } else if x == right {
            line::VERTICAL_LEFT
        } else {
            line::HORIZONTAL
        };

        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.reset();
            cell.set_symbol(symbol).set_style(style);
        }
    }
}

/// Writes `parts` in turn over a border row of `pane` from column `x`, stopping
/// short of its right side. A Block title would allocate on every frame.
pub fn label<'a>(
    buf: &mut Buffer,
    pane: Rect,
    (x, y): (u16, u16),
    parts: impl IntoIterator<Item = &'a str>,
    style: Style,
) {
    let mut x = x;
    for part in parts {
        let room = usize::from(pane.right().saturating_sub(x + 1));
        x = buf.set_stringn(x, y, part, room, style).0;
    }
}

/// A space, `text` and `suffix` over a border row in `row`, `text` cut with
/// `…` so that `suffix` still fits.
pub fn title(buf: &mut Buffer, row: Rect, text: &str, suffix: &str, style: Style) {
    let (x, _) = buf.set_stringn(row.x, row.y, " ", row.width.into(), style);
    let suffix_width = u16::try_from(suffix.width()).unwrap_or(u16::MAX);
    let room = row.right().saturating_sub(x).saturating_sub(suffix_width);
    let x = truncate::draw(buf, Rect::new(x, row.y, room, 1), text, style);
    let room = usize::from(row.right().saturating_sub(x));
    buf.set_stringn(x, row.y, suffix, room, style);
}
