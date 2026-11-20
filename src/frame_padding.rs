use crate::{
    frame_glyphs::{BOTTOM_CORNERS, TOP_CORNERS},
    styled_line::StyledLine,
};

/// Drops the blank row a card pads itself with under the top of its frame
/// and over the bottom: the frame sets the card apart already.
pub fn trim(lines: &mut Vec<StyledLine>) {
    let edge = |row: Option<usize>, corners: &[char]| {
        row.and_then(|row| lines.get(row))
            .is_some_and(|line| line.frame_edge(corners))
    };

    let padding: Vec<bool> = (0..lines.len())
        .map(|row| {
            lines[row].spacer()
                && (edge(row.checked_sub(1), &TOP_CORNERS) || edge(Some(row + 1), &BOTTOM_CORNERS))
        })
        .collect();
    let mut padding = padding.iter();
    lines.retain(|_| padding.next() == Some(&false));
}
