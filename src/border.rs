use ratatui::{buffer::Buffer, layout::Rect, style::Style, symbols::line};

/// `├───┤` across the bordered `pane` on row `y`, joined to its sides.
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
            cell.set_symbol(symbol).set_style(style);
        }
    }
}

/// Writes `parts` in turn over a border row of `pane` from column `x`, stopping
/// short of its right side. A Block title would allocate on every frame.
pub fn label(buf: &mut Buffer, pane: Rect, (x, y): (u16, u16), parts: &[&str], style: Style) {
    let mut x = x;
    for part in parts {
        let room = usize::from(pane.right().saturating_sub(x + 1));
        x = buf.set_stringn(x, y, part, room, style).0;
    }
}
