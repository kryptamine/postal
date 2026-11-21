//! The glyphs frames are drawn with in the rendered View.

/// The corners a frame's top edge turns down at, square or round.
pub const TOP_CORNERS: [char; 4] = ['┌', '┐', '╭', '╮'];
/// The corners a frame's bottom edge turns up at.
pub const BOTTOM_CORNERS: [char; 4] = ['└', '┘', '╰', '╯'];

pub fn piece(c: char) -> bool {
    c == '─' || c == '│' || TOP_CORNERS.contains(&c) || BOTTOM_CORNERS.contains(&c)
}

/// `glyph`, a square corner, as the round one; any other piece as it is.
pub fn rounded(glyph: &'static str) -> &'static str {
    match glyph {
        "┌" => "╭",
        "┐" => "╮",
        "└" => "╰",
        "┘" => "╯",
        _ => glyph,
    }
}
