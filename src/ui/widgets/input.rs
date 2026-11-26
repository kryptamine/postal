use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
};

use crate::ui::widgets::truncate;

/// The end of `text` that fits `width` columns with one left for the cursor,
/// so what is being typed stays in view.
pub fn shown(text: &str, width: u16) -> &str {
    truncate::tail(text, usize::from(width).saturating_sub(1))
}

/// Writes `shown(text)` on the area's first row; returns the column after
/// it, where the cursor goes, or `None` when `area` is empty.
pub fn draw(buf: &mut Buffer, area: Rect, text: &str, style: Style) -> Option<Position> {
    if area.is_empty() {
        return None;
    }

    let shown = shown(text, area.width);
    let (x, _) = buf.set_stringn(area.x, area.y, shown, area.width.into(), style);
    Some(Position::new(x, area.y))
}
