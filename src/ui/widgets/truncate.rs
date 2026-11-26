use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Writes `text` on the area's first row, cut to its width and ending in
/// `…` when cut; returns the column after it.
pub fn draw(buf: &mut Buffer, area: Rect, text: &str, style: Style) -> u16 {
    if area.is_empty() {
        return area.x;
    }

    let width = usize::from(area.width);
    let (shown, cut) = fit(text, width);
    let (end, _) = buf.set_stringn(area.x, area.y, shown, width, style);
    if cut {
        return buf.set_stringn(end, area.y, "…", 1, style).0;
    }

    end
}

/// The longest suffix of `text` that fits `width` columns, cut on a grapheme
/// boundary.
pub fn tail(text: &str, width: usize) -> &str {
    if text.width() <= width {
        return text;
    }

    let mut used = 0;
    let mut start = text.len();
    for (at, grapheme) in text.grapheme_indices(true).rev() {
        used += grapheme.width();
        if used > width {
            break;
        }

        start = at;
    }

    text.split_at(start).1
}

/// The longest prefix of `text` that fits `width` columns, cut on a grapheme
/// boundary, and whether it was cut. A cut prefix leaves one column for `…`.
pub fn fit(text: &str, width: usize) -> (&str, bool) {
    if text.width() <= width {
        return (text, false);
    }

    let budget = width.saturating_sub(1);
    let mut used = 0;
    let mut end = 0;
    for (start, grapheme) in text.grapheme_indices(true) {
        used += grapheme.width();
        if used > budget {
            break;
        }

        end = start + grapheme.len();
    }

    (text.split_at(end).0, true)
}
