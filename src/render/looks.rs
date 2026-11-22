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
}

impl Looks {
    /// `outlines` says what became of the fill under each cell of `grid`.
    pub fn new(grid: &CellGrid, outlines: &[Outline], colours: MailColours) -> Self {
        let on_terminal = grid
            .cells()
            .iter()
            .zip(outlines)
            .filter(|&(cell, &outline)| worded(cell) && fill(cell, outline, colours).is_none())
            .filter_map(|(cell, _)| Some((cell.fg()?, under(cell))));
        Self {
            colours,
            palette: Palette::new(on_terminal),
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
                Role::Ink => (None, plain),
                Role::Muted => (None, Marks::MUTED),
                Role::Accent(accent) => (Some(self.colours.text(accent, None)), plain),
            },
        }
    }
}

/// A glyph of the mail's own text: not a blank, a border or an alt.
fn worded(cell: &Cell) -> bool {
    cell.has(cell_grid::TEXT)
        && !cell.has(cell_grid::BORDER | cell_grid::ALT | cell_grid::WIDE_TAIL)
        && cell.glyph() != " "
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
