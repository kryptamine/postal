use std::collections::{HashMap, HashSet};

use crate::{
    render::cell_grid::{self, Cell, CellGrid},
    render::mail_colours::MailColours,
    render::marks::Marks,
    render::outlines::Outline,
    render::paint::Paint,
    render::palette::{Palette, Role},
    render::rgb::{Rgb, WHITE},
};

/// The least contrast white needs on the fill of a button or a badge to
/// be its ink, whatever the mail set: brands label one in black, the next
/// in white. Under it the mail's colour stays, black on a yellow button.
const CONTROL_INK: f32 = 2.5;

/// How each cell of one laid out mail is drawn: its marks, and its colours
/// moved onto the terminal's background.
pub struct Looks {
    colours: MailColours,
    palette: Palette,
    /// Single-line inline links in plain ink with no CSS underline or fill.
    inline_links: HashSet<u32>,
}

impl Looks {
    /// `outlines` says what became of the fill under each cell of `grid`.
    pub fn new(grid: &CellGrid, outlines: &[Outline], colours: MailColours) -> Self {
        let on_terminal = grid
            .cells()
            .iter()
            .zip(outlines)
            .filter(|&(cell, &outline)| on_terminal(cell, outline, colours))
            .filter_map(|(cell, _)| Some((cell.fg()?, under(cell))));
        let palette = Palette::new(on_terminal);
        let inline_links = inline_ink_links(grid, outlines, colours, &palette);
        Self {
            colours,
            palette,
            inline_links,
        }
    }

    /// A blank cell takes the look of the cell `before` it on the same
    /// background, so the words either side of it stay one run.
    pub fn of(&self, cell: &Cell, outline: Outline, before: (Marks, Paint)) -> (Marks, Paint) {
        match outline {
            Outline::Line(_, colour) => {
                let (fg, marks) = self.colours.line(Some(colour), None);
                return (marks, Paint { fg, bg: None });
            }
            Outline::Cap(_) => {
                let fg = fill(cell, Outline::Fill, self.colours);
                return (Marks::default(), Paint { fg, bg: None });
            }
            Outline::Fill | Outline::Bare => {}
        }

        let bg = fill(cell, outline, self.colours);
        if cell.has(cell_grid::BORDER) {
            let (fg, marks) = self.colours.line(cell.fg(), bg);
            return (marks, Paint { fg, bg });
        }

        if cell.has(cell_grid::ALT) {
            return (Marks::ALT.with(Marks::EMPHASIS), Paint { fg: None, bg });
        }

        let underline = (
            cell.has(cell_grid::UNDERLINE) || self.inline_links.contains(&cell.link()),
            Marks::UNDERLINE,
        );
        if cell.glyph() == " " && !underline.0 {
            if before.1.bg == bg && !before.0.has(Marks::UNDERLINE) {
                return before;
            }

            return (Marks::default(), Paint { fg: None, bg });
        }

        let marks = [
            (cell.has(cell_grid::BOLD), Marks::STRONG),
            (cell.has(cell_grid::ITALIC), Marks::EMPHASIS),
            (cell.has(cell_grid::STRIKE), Marks::STRIKE),
            underline,
        ]
        .into_iter()
        .filter(|&(on, _)| on)
        .fold(Marks::default(), |marks, (_, mark)| marks.with(mark));
        if cell.has(cell_grid::SMALL) && bg.is_none() {
            return (marks.with(Marks::MUTED), Paint { fg: None, bg });
        }

        let (fg, role) = self.text(cell, bg);
        (marks.with(role), Paint { fg, bg })
    }

    /// The fg of the text in `cell`, and the mark of its role. On a fill
    /// left in place it is read against that fill; on the terminal's
    /// background it has a role instead of the mail's colour.
    fn text(&self, cell: &Cell, bg: Option<Rgb>) -> (Option<Rgb>, Marks) {
        let plain = Marks::default();
        match (cell.fg(), bg) {
            (None, _) => (None, plain),
            (Some(_), Some(fill)) if !cell.panel() && WHITE.contrast(fill) >= CONTROL_INK => {
                (Some(WHITE), plain)
            }
            (Some(fg), Some(_)) => (Some(self.colours.text(fg, bg)), plain),
            (Some(fg), None) => match self.palette.role(fg, under(cell)) {
                // Ink set larger than the body is a heading.
                Role::Ink if cell.has(cell_grid::TITLE) => (None, Marks::HEADING),
                Role::Ink if cell.has(cell_grid::CODE) => (None, Marks::CODE),
                Role::Ink => (None, plain),
                Role::Muted => (None, Marks::MUTED),
                Role::Accent(accent) => (Some(self.colours.text(accent, None)), plain),
            },
        }
    }
}

/// Links that sit on one row beside plain words, in ordinary ink with no fill
/// or colour of their own: without an underline they are invisible in prose.
fn inline_ink_links(
    grid: &CellGrid,
    outlines: &[Outline],
    colours: MailColours,
    palette: &Palette,
) -> HashSet<u32> {
    let cols = usize::from(grid.cols().max(1));
    let mut row_has_prose = vec![false; grid.cells().len().div_ceil(cols)];
    // Each link's one row, or `None` once it fails a test.
    let mut links: HashMap<u32, Option<usize>> = HashMap::new();
    for (row_idx, (row, row_outlines)) in grid.rows().zip(outlines.chunks(cols)).enumerate() {
        for (cell, &outline) in row.iter().zip(row_outlines) {
            if !worded(cell) {
                continue;
            }

            let id = cell.link();
            if id == 0 {
                if cell.glyph().chars().any(char::is_alphanumeric) {
                    row_has_prose[row_idx] = true;
                }

                continue;
            }

            let plain_ink = on_terminal(cell, outline, colours)
                && !cell.has(cell_grid::TITLE | cell_grid::SMALL)
                && cell
                    .fg()
                    .is_some_and(|fg| matches!(palette.role(fg, under(cell)), Role::Ink));
            let entry = links.entry(id).or_insert(Some(row_idx));
            if *entry != Some(row_idx) || !plain_ink {
                *entry = None;
            }
        }
    }

    links
        .into_iter()
        .filter_map(|(id, row)| row.filter(|&row| row_has_prose[row]).map(|_| id))
        .collect()
}

/// A glyph of the mail's own text: not a blank, a border or an alt.
fn worded(cell: &Cell) -> bool {
    cell.has(cell_grid::TEXT)
        && !cell.has(cell_grid::BORDER | cell_grid::ALT | cell_grid::WIDE_TAIL)
        && cell.glyph() != " "
}

/// Text the terminal's background shows through: no fill is left under it.
fn on_terminal(cell: &Cell, outline: Outline, colours: MailColours) -> bool {
    worded(cell) && fill(cell, outline, colours).is_none()
}

/// The fill left under `cell`, moved onto the terminal's background.
fn fill(cell: &Cell, outline: Outline, colours: MailColours) -> Option<Rgb> {
    if outline == Outline::Fill {
        colours.background(cell.bg(), cell.panel())
    } else {
        None
    }
}

/// What the mail put under the text of `cell`: its page is white.
fn under(cell: &Cell) -> Rgb {
    cell.bg().unwrap_or(WHITE)
}
