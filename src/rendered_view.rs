use std::{collections::HashMap, sync::Arc};

use crate::{
    cell_grid::{self, Cell, CellGrid},
    mail_colours::MailColours,
    marks::Marks,
    paint::Paint,
    render_process,
    styled_line::StyledLine,
};

/// An HTML part laid out as a mail client shows it, `width` cells wide,
/// in `colours`; `None` when the engine fails on it, or when it is too wide
/// to show whole. Blank rows at the ends are dropped.
pub fn render(html: &str, width: u16, colours: MailColours) -> Option<Vec<StyledLine>> {
    let grid = render_process::lay_out(html, width).filter(CellGrid::fits)?;
    let mut hrefs: HashMap<u32, Arc<str>> = HashMap::new();
    let mut lines: Vec<StyledLine> = Vec::new();
    for row in grid.rows() {
        let mut line = StyledLine::new();
        let mut last = (Marks::default(), Paint::default());
        for cell in row.iter().filter(|cell| !cell.has(cell_grid::WIDE_TAIL)) {
            last = look(cell, colours, last);
            let (marks, paint) = last;
            let target = match cell.link() {
                0 => None,
                id => grid
                    .href(id)
                    .map(|href| &*hrefs.entry(id).or_insert_with(|| Arc::from(href))),
            };

            line.push(cell.glyph(), marks, paint, target);
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
    Some(lines)
}

/// A blank cell takes the look of the cell before it on the same
/// background, so the words either side of it stay one run.
fn look(cell: &Cell, colours: MailColours, before: (Marks, Paint)) -> (Marks, Paint) {
    let bg = colours.background(cell.bg());
    if cell.has(cell_grid::BORDER) {
        let (fg, marks) = colours.line(cell.fg(), bg);
        return (marks, Paint { fg, bg });
    }

    if cell.has(cell_grid::ALT) {
        return (Marks::ALT.with(Marks::EMPHASIS), Paint { fg: None, bg });
    }

    let underline = (cell.has(cell_grid::UNDERLINE), Marks::UNDERLINE);
    if cell.glyph() == " " && !underline.0 {
        if before.1.bg == bg && !before.0.has(Marks::UNDERLINE) {
            return before;
        }

        return (Marks::default(), Paint { fg: None, bg });
    }

    let marks = [
        (cell.has(cell_grid::BOLD), Marks::STRONG),
        (cell.has(cell_grid::ITALIC), Marks::EMPHASIS),
        underline,
    ]
    .into_iter()
    .filter(|&(on, _)| on)
    .fold(Marks::default(), |marks, (_, mark)| marks.with(mark));
    let fg = colours.text(cell.fg(), bg);
    (marks, Paint { fg, bg })
}
