use crate::styled_line::StyledLine;

/// Drops the blank row a card pads itself with under the top of its frame
/// and over the bottom: the frame sets the card apart already.
pub fn trim(lines: &mut Vec<StyledLine>) {
    let edge = |row: Option<usize>, corner| {
        row.and_then(|row| lines.get(row))
            .is_some_and(|line| line.frame_edge(corner))
    };

    let padding: Vec<bool> = (0..lines.len())
        .map(|row| {
            lines[row].spacer() && (edge(row.checked_sub(1), '┌') || edge(Some(row + 1), '└'))
        })
        .collect();
    let mut padding = padding.iter();
    lines.retain(|_| padding.next() == Some(&false));
}
