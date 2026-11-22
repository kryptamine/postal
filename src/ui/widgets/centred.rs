use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use unicode_width::UnicodeWidthStr;

/// Writes `text` centred on the middle row of `area`. Unlike a `Paragraph`
/// it does not allocate, so a frame may draw it.
pub fn centred(text: &str, area: Rect, buf: &mut Buffer) {
    if area.is_empty() {
        return;
    }

    let width = u16::try_from(text.width())
        .unwrap_or(u16::MAX)
        .min(area.width);
    let x = area.x + (area.width - width) / 2;
    let y = area.y + area.height / 2;
    buf.set_stringn(x, y, text, usize::from(area.width), Style::new());
}
