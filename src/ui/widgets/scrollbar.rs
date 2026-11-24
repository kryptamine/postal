use ratatui::{buffer::Buffer, layout::Rect, style::Style};

const THUMB: &str = "▐";

/// A thumb down `track` as long as the share of `total` rows it shows,
/// placed as far down as `scroll`, the first row shown, has gone.
pub fn scrollbar(buf: &mut Buffer, track: Rect, (scroll, total): (usize, usize), style: Style) {
    let height = usize::from(track.height);
    if total <= height || height == 0 {
        return;
    }

    let thumb = (height * height / total).clamp(1, height);
    let top = (height - thumb) * scroll.min(total - height) / (total - height);
    for (row, y) in (track.y..track.bottom()).enumerate() {
        if (top..top + thumb).contains(&row) {
            buf.set_string(track.x, y, THUMB, style);
        }
    }
}
