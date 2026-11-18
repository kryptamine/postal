use crate::{
    cell_grid::CellGrid,
    region::{self, Region},
    rgb::Rgb,
};

/// What becomes of the fill under a cell.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Outline {
    /// It stays as the mail gave it.
    Fill,
    /// It goes, and the terminal's background shows.
    Bare,
    /// It goes, and this piece of the frame round its region is drawn, in
    /// the fill's colour: a frame moved off text lies on cells with no fill.
    Line(&'static str, Rgb),
}

/// For each cell of `grid`: a section's fill gives way to a frame in its
/// colour round every region of one fill, so text sits on the terminal's
/// own background. See docs/adr/0006-rendered-view-engine.md.
pub fn of(grid: &CellGrid) -> Vec<Outline> {
    let cells = grid.cells();
    let mut out = vec![Outline::Fill; cells.len()];
    let mut seen = vec![false; cells.len()];
    let page = region::page(grid);
    let fills: Vec<Option<Rgb>> = cells.iter().map(|cell| region::fill(cell, page)).collect();
    for start in 0..cells.len() {
        let Some(colour) = fills[start].filter(|_| !seen[start]) else {
            continue;
        };

        let members = touching(&fills, grid.cols(), start, &mut seen);
        let region = Region::new(grid, &members, colour);
        let is_page = region.page(page);
        if !is_page && !region.boxed() {
            continue;
        }

        for &i in &members {
            out[i] = Outline::Bare;
        }

        if is_page || region.bordered() {
            continue;
        }

        for (i, glyph) in region.frame(|i| out[i] == Outline::Fill) {
            out[i] = Outline::Line(glyph, colour);
        }
    }

    out
}

/// The cells joined to `start` through cells of its fill, marked `seen`.
fn touching(fills: &[Option<Rgb>], cols: u16, start: usize, seen: &mut [bool]) -> Vec<usize> {
    let cols = usize::from(cols.max(1));
    let colour = fills[start];
    let mut members = vec![start];
    seen[start] = true;
    let mut next = 0;
    while let Some(&i) = members.get(next) {
        next += 1;
        let col = i % cols;
        let around = [
            i.checked_sub(cols),
            Some(i + cols).filter(|&below| below < fills.len()),
            (col > 0).then(|| i - 1),
            (col + 1 < cols).then(|| i + 1),
        ];

        for n in around.into_iter().flatten() {
            if !seen[n] && fills[n] == colour {
                seen[n] = true;
                members.push(n);
            }
        }
    }

    members
}
