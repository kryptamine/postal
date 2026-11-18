use crate::{
    cell_grid::{self, Cell, CellGrid},
    rgb::{Rgb, WHITE},
};

/// A region this short or narrow has no room for a frame.
const LEAST: usize = 3;
/// The cells left round the bounding box: one to move an edge out into,
/// and one past it that stays outside.
const RING: usize = 2;
/// From a cell of a window `width` wide to the one above it, below it, on
/// its left and on its right.
const SIDES: [fn(usize, usize) -> usize; 4] = [
    |at, width| at - width,
    |at, width| at + width,
    |at, _| at - 1,
    |at, _| at + 1,
];

/// A light fill this far from the page is a card laid on it; any nearer
/// only tints it.
const TINT: u8 = 10;

/// The section fill of `cell`: one dark enough not to be the page, or a
/// light one set apart from the mail's `page`.
pub fn fill(cell: &Cell, page: Rgb) -> Option<Rgb> {
    cell.bg()
        .filter(|&bg| cell.panel() && (!page_light(bg) || bg.distance(page) >= TINT))
}

/// The colour most of the pane's side columns lie on, white where the mail
/// sets none: cards fill the middle, and may outnumber the page there.
pub fn page(grid: &CellGrid) -> Rgb {
    let sides = grid
        .rows()
        .flat_map(|row| [row.first(), row.last()])
        .flatten();
    let mut counts: Vec<(Rgb, usize)> = Vec::new();
    for cell in sides {
        let bg = cell.bg().unwrap_or(WHITE);
        match counts.iter_mut().find(|(colour, _)| *colour == bg) {
            Some((_, count)) => *count += 1,
            None => counts.push((bg, 1)),
        }
    }

    counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map_or(WHITE, |(colour, _)| colour)
}

/// The cells of one section fill that touch, in a window `RING` cells
/// wider than their bounding box on every side.
pub struct Region<'a> {
    cells: &'a [Cell],
    cols: usize,
    /// The grid row and column of the bounding box's first cell.
    top: usize,
    left: usize,
    height: usize,
    width: usize,
    inside: Vec<bool>,
    colour: Rgb,
    boxed: bool,
}

impl<'a> Region<'a> {
    /// `members` are indices into the cells of `grid`, at least one, all
    /// filled with `colour`.
    pub fn new(grid: &'a CellGrid, members: &[usize], colour: Rgb) -> Self {
        let cols = usize::from(grid.cols().max(1));
        let row = |&i: &usize| i / cols;
        let col = |&i: &usize| i % cols;
        let top = members.iter().map(row).min().unwrap_or(0);
        let left = members.iter().map(col).min().unwrap_or(0);
        let bottom = members.iter().map(row).max().unwrap_or(0);
        let right = members.iter().map(col).max().unwrap_or(0);
        let (height, width) = (bottom - top + 1 + 2 * RING, right - left + 1 + 2 * RING);
        let mut inside = vec![false; height * width];
        for i in members {
            inside[(row(i) - top + RING) * width + col(i) - left + RING] = true;
        }

        let mut region = Self {
            cells: grid.cells(),
            cols,
            top,
            left,
            height,
            width,
            inside,
            colour,
            boxed: false,
        };
        region.boxed = region.rectangle();
        region
    }

    /// A rectangle with room for a frame. Any other shape is a drawing made
    /// of fills, as the bars of a chart are, and a frame would only break it.
    pub const fn boxed(&self) -> bool {
        self.boxed
    }

    fn rectangle(&self) -> bool {
        let outside = self.outside();
        let (rows, cols) = (RING..self.height - RING, RING..self.width - RING);
        let filled = |row: usize| cols.clone().all(|col| !outside[row * self.width + col]);
        rows.len() >= LEAST && cols.len() >= LEAST && rows.into_iter().all(filled)
    }

    /// The mail's own page, which a frame would only box whole: as wide as
    /// the pane and four fifths of the mail high, or in the `page` colour
    /// round a narrower card. A rectangle that wide is a section.
    pub fn page(&self, page: Rgb) -> bool {
        let rows = self.cells.len() / self.cols;
        let in_page = self.colour == page;
        let right = self.left + self.width - 2 * RING == self.cols;
        match (self.left == 0, right) {
            (true, true) => (self.height - 2 * RING) * 5 >= rows * 4 || (in_page && !self.boxed),
            (true, false) | (false, true) => in_page,
            (false, false) => false,
        }
    }

    /// Most of its edge lies against a border the mail drew itself.
    pub fn bordered(&self) -> bool {
        let lined = |side: usize, at: usize| {
            self.cell(SIDES[side](at, self.width))
                .is_some_and(|(_, cell)| cell.has(cell_grid::BORDER))
        };

        let (mut against, mut all) = (0, 0);
        for (side, run) in self.edges() {
            all += run.len();
            against += run.iter().filter(|&&at| lined(side, at)).count();
        }

        against * 2 > all
    }

    /// The frame: each grid cell it is drawn in, and its glyph. An edge with
    /// text on it moves a cell out, where every cell there is blank, has no
    /// fill and is `unclaimed`; where not, a level edge is not drawn.
    pub fn frame(mut self, unclaimed: impl Fn(usize) -> bool) -> Vec<(usize, &'static str)> {
        let free = |region: &Self, at: usize| {
            region.cell(at).is_some_and(|(i, cell)| {
                unclaimed(i) && cell.bg().is_none_or(page_light) && blank(cell)
            })
        };

        let mut flat = vec![false; self.inside.len()];
        let mut moved = Vec::new();
        for (side, run) in self.edges() {
            let text = |&at: &usize| self.cell(at).is_some_and(|(_, cell)| taken(cell));
            if !run.iter().any(text) {
                continue;
            }

            let beyond = |&at: &usize| SIDES[side](at, self.width);
            if run.iter().map(beyond).all(|at| free(&self, at)) {
                moved.extend(run.iter().map(beyond));
            } else if side < 2 {
                for at in run {
                    flat[at] = true;
                }
            }
        }

        for at in moved {
            self.inside[at] = true;
        }

        let outside = self.outside();
        (0..self.inside.len())
            .filter(|&at| self.inside[at])
            .filter_map(|at| {
                let (i, _) = self.cell(at).filter(|(_, cell)| !taken(cell))?;
                let glyph = glyph(&outside, at, self.width)?;
                let upright = outside[at - 1] || outside[at + 1];
                match (flat[at], upright) {
                    (false, _) => Some((i, glyph)),
                    (true, true) => Some((i, "│")),
                    (true, false) => None,
                }
            })
            .collect()
    }

    /// The grid cell under window cell `at`, and its index, where the
    /// window has not left the grid.
    fn cell(&self, at: usize) -> Option<(usize, &'a Cell)> {
        let row = (at / self.width + self.top).checked_sub(RING)?;
        let col = (at % self.width + self.left).checked_sub(RING)?;
        let i = row * self.cols + col;
        self.cells
            .get(i)
            .filter(|_| col < self.cols)
            .map(|cell| (i, cell))
    }

    /// The window cells reached from its corner without crossing the
    /// region: a hole in the region is not among them.
    fn outside(&self) -> Vec<bool> {
        let mut outside = vec![false; self.inside.len()];
        let mut stack = vec![0];
        outside[0] = true;
        while let Some(at) = stack.pop() {
            let (row, col) = (at / self.width, at % self.width);
            let around = [
                (row > 0).then(|| at - self.width),
                (row + 1 < self.height).then(|| at + self.width),
                (col > 0).then(|| at - 1),
                (col + 1 < self.width).then(|| at + 1),
            ];

            for next in around.into_iter().flatten() {
                if !outside[next] && !self.inside[next] {
                    outside[next] = true;
                    stack.push(next);
                }
            }
        }

        outside
    }

    /// Each unbroken run of cells with the outside on one side, with the
    /// index of that side in `SIDES`.
    fn edges(&self) -> Vec<(usize, Vec<usize>)> {
        let outside = self.outside();
        let len = self.inside.len();
        let mut edges = Vec::new();
        for side in 0..SIDES.len() {
            let mut run = Vec::new();
            for n in 0..len {
                // Level edges run along rows, upright ones down columns.
                let at = if side < 2 {
                    n
                } else {
                    n % self.height * self.width + n / self.height
                };

                if self.inside[at] && outside[SIDES[side](at, self.width)] {
                    run.push(at);
                } else if !run.is_empty() {
                    edges.push((side, std::mem::take(&mut run)));
                }
            }
        }

        edges
    }
}

fn page_light(bg: Rgb) -> bool {
    bg.luminance() > cell_grid::PAGE_LUMINANCE
}

/// Nothing of the mail's shows in `cell`, not even a space of its text.
fn blank(cell: &Cell) -> bool {
    !taken(cell) && !cell.has(cell_grid::TEXT)
}

/// A frame cannot be drawn in `cell`: it shows something of the mail's.
fn taken(cell: &Cell) -> bool {
    let shown = cell_grid::ALT | cell_grid::UNDERLINE | cell_grid::WIDE_TAIL | cell_grid::BORDER;
    cell.glyph() != " " || cell.has(shown)
}

/// The piece of frame in window cell `at`, by where the outside lies; a
/// cell with the outside only at a corner is where two edges turn inward.
fn glyph(outside: &[bool], at: usize, width: usize) -> Option<&'static str> {
    let beside = |side: usize| outside[SIDES[side](at, width)];
    Some(match (beside(0), beside(1), beside(2), beside(3)) {
        (true, true, _, _) => "─",
        (_, _, true, true) => "│",
        (true, _, true, _) => "┌",
        (true, _, _, true) => "┐",
        (_, true, true, _) => "└",
        (_, true, _, true) => "┘",
        (true, ..) | (_, true, ..) => "─",
        (_, _, true, _) | (_, _, _, true) => "│",
        _ if outside[at - width - 1] => "┘",
        _ if outside[at - width + 1] => "└",
        _ if outside[at + width - 1] => "┐",
        _ if outside[at + width + 1] => "┌",
        _ => return None,
    })
}
