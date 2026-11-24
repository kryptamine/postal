use ratatui::layout::Rect;

/// The tallest a popup grows, so a long list scrolls in a box the eye
/// takes in rather than one spanning the terminal.
const MAX_HEIGHT: u16 = 30;

/// `width` by `height` centred over `area`, no taller than it nor than
/// `MAX_HEIGHT`.
pub fn popup_area(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height).min(MAX_HEIGHT);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}
