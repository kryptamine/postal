use crate::styled_line::StyledLine;

/// Keeps one of each run of equal rows that show nothing but the sides of
/// the frames they cross: mail spaces its blocks with rows of padding, and
/// one row sets them apart as well as five. A row with a fill is a drawing.
pub fn squeeze(lines: &mut Vec<StyledLine>) {
    lines.dedup_by(|line, kept| spacer(kept) && line == kept);
}

fn spacer(line: &StyledLine) -> bool {
    line.segments().all(|segment| {
        segment.paint.bg.is_none() && segment.text.chars().all(|c| c == ' ' || c == '│')
    })
}
