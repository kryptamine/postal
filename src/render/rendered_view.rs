use std::{collections::HashMap, sync::Arc};

use crate::{
    render::cell_grid::{self, CellGrid},
    render::footer,
    render::frame_padding,
    render::looks::Looks,
    render::mail_look::MailLook,
    render::marks::Marks,
    render::outlines::{self, Outline},
    render::paint::Paint,
    render::render_process,
    render::styled_line::StyledLine,
};

/// An HTML part laid out as a mail client shows it, `width` cells wide, in
/// `look`; `None` when the engine fails on it or it is too wide to show
/// whole. Blank rows at the ends are dropped, and a run of them inside is one.
pub fn render(html: &str, width: u16, look: MailLook) -> Option<Vec<StyledLine>> {
    let grid = render_process::lay_out(html, width).filter(CellGrid::fits)?;
    let mut hrefs: HashMap<u32, Arc<str>> = HashMap::new();
    let mut lines: Vec<StyledLine> = Vec::new();
    let outlines = outlines::of(&grid, look.icons);
    let looks = Looks::new(&grid, &outlines, look.colours);
    let mut outline = outlines.iter().copied();
    for row in grid.rows() {
        let mut line = StyledLine::new();
        let mut last = (Marks::default(), Paint::default());
        for (&cell, outline) in row.iter().zip(outline.by_ref()) {
            let cell = &cell.unframed();
            if cell.has(cell_grid::WIDE_TAIL) {
                continue;
            }

            last = looks.of(cell, outline, last);
            let (marks, paint) = last;
            let target = match (outline, cell.link()) {
                (Outline::Line(..), _) | (_, 0) => None,
                (_, id) => grid
                    .href(id)
                    .map(|href| &*hrefs.entry(id).or_insert_with(|| Arc::from(href))),
            };

            let glyph = match outline {
                Outline::Line(glyph, _) | Outline::Cap(glyph) => glyph,
                Outline::Fill | Outline::Bare => cell.glyph(),
            };

            line.push(glyph, marks, paint, target);
        }

        line.trim_end();
        lines.push(line);
    }

    let blank = |line: &StyledLine| line.text.is_empty();
    let end = lines
        .iter()
        .rposition(|line| !blank(line))
        .map_or(0, |last| last + 1);
    lines.truncate(end);
    let start = lines.iter().position(|line| !blank(line)).unwrap_or(0);
    lines.drain(..start);
    // Mail spaces its blocks with rows of padding, and one row sets them
    // apart as well as five. Blank rows are alike by their text: what marks
    // their cells carry shows nowhere.
    lines.dedup_by(|line, kept| kept.spacer() && line.spacer() && line.text == kept.text);
    frame_padding::trim(&mut lines);
    footer::mute(&mut lines);
    Some(lines)
}
