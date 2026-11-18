//! HTML laid out on the terminal cell grid, and the bytes it crosses from
//! the layout process in. See docs/adr/0006-rendered-view-engine.md.

use crate::rgb::Rgb;

pub const BOLD: u32 = 1;
pub const ITALIC: u32 = 2;
pub const UNDERLINE: u32 = 4;
pub const TEXT: u32 = 8;
/// The right half of a wide glyph, which prints nothing.
pub const WIDE_TAIL: u32 = 16;
pub const BORDER: u32 = 32;
/// An image's alt text, drawn in its place.
pub const ALT: u32 = 64;
/// In `bg`: it comes from a section or the page, not from a control.
const BG_PANEL: u32 = 0x0200_0000;
/// A background this light is the mail's page, which becomes the terminal's:
/// about `#cccccc` and up, in WCAG's linear luminance. The layout reads it too.
pub const PAGE_LUMINANCE: f32 = 0.6;
/// A colour whose channels spread less than this is a grey.
pub const GREY_SPREAD: u8 = 40;

/// One cell as `cpp/cell_grid.h` lays it out.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Cell {
    glyph: [u8; GLYPH],
    fg: u32,
    bg: u32,
    flags: u32,
    link: u32,
}

const GLYPH: usize = 16;
/// The glyph, then fg, bg, flags and link, little-endian.
const CELL_BYTES: usize = GLYPH + 4 * 4;

impl Cell {
    pub fn glyph(&self) -> &str {
        let end = self.glyph.iter().position(|&b| b == 0).unwrap_or(GLYPH);
        std::str::from_utf8(&self.glyph[..end]).unwrap_or("\u{fffd}")
    }

    pub const fn fg(&self) -> Option<Rgb> {
        colour(self.fg)
    }

    pub const fn bg(&self) -> Option<Rgb> {
        colour(self.bg)
    }

    /// The background comes from a section or the page, not from a control.
    pub const fn panel(&self) -> bool {
        self.bg & BG_PANEL != 0
    }

    pub const fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    /// The id of the link the cell is part of; 0 for none.
    pub const fn link(&self) -> u32 {
        self.link
    }
}

/// `0x01RRGGBB` when set, 0 when not.
#[allow(
    clippy::cast_possible_truncation,
    reason = "each channel is masked to one byte"
)]
const fn colour(packed: u32) -> Option<Rgb> {
    if packed == 0 {
        return None;
    }

    Some(Rgb {
        r: (packed >> 16) as u8,
        g: (packed >> 8) as u8,
        b: packed as u8,
    })
}

/// A laid out document: rows of `cols` cells, and the href of each link id.
pub struct CellGrid {
    cols: u16,
    cells: Vec<Cell>,
    hrefs: Vec<String>,
    /// No text fell past the last column, as it does from a table too wide.
    fits: bool,
}

impl CellGrid {
    pub const fn new(cols: u16, cells: Vec<Cell>, hrefs: Vec<String>, fits: bool) -> Self {
        Self {
            cols,
            cells,
            hrefs,
            fits,
        }
    }

    pub const fn fits(&self) -> bool {
        self.fits
    }

    pub const fn cols(&self) -> u16 {
        self.cols
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn rows(&self) -> impl Iterator<Item = &[Cell]> {
        self.cells.chunks(usize::from(self.cols.max(1)))
    }

    pub fn href(&self, link: u32) -> Option<&str> {
        let index = usize::try_from(link).ok()?.checked_sub(1)?;
        self.hrefs.get(index).map(String::as_str)
    }

    /// The cols, whether it fits, the cell count, each cell, the href count,
    /// and each href after its length, every number a little-endian `u32`.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.cells.len() * CELL_BYTES + 16);
        let number = |out: &mut Vec<u8>, n: usize| {
            out.extend_from_slice(&u32::try_from(n).unwrap_or(u32::MAX).to_le_bytes());
        };

        number(&mut out, usize::from(self.cols));
        number(&mut out, usize::from(self.fits));
        number(&mut out, self.cells.len());
        for cell in &self.cells {
            out.extend_from_slice(&cell.glyph);
            for field in [cell.fg, cell.bg, cell.flags, cell.link] {
                out.extend_from_slice(&field.to_le_bytes());
            }
        }

        number(&mut out, self.hrefs.len());
        for href in &self.hrefs {
            number(&mut out, href.len());
            out.extend_from_slice(href.as_bytes());
        }

        out
    }

    /// `None` when `bytes` is not what `encode` writes.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut input = Input(bytes);
        let cols = u16::try_from(input.number()?).ok()?;
        let fits = input.number()? != 0;
        let count = usize::try_from(input.number()?).ok()?;
        let mut cells = Vec::with_capacity(count.min(bytes.len() / CELL_BYTES));
        for _ in 0..count {
            let glyph = input.take(GLYPH)?.try_into().ok()?;
            cells.push(Cell {
                glyph,
                fg: input.number()?,
                bg: input.number()?,
                flags: input.number()?,
                link: input.number()?,
            });
        }

        let links = input.number()?;
        let hrefs = (0..links)
            .map(|_| {
                let len = usize::try_from(input.number()?).ok()?;
                String::from_utf8(input.take(len)?.to_vec()).ok()
            })
            .collect::<Option<Vec<_>>>()?;

        input.0.is_empty().then_some(Self {
            cols,
            cells,
            hrefs,
            fits,
        })
    }
}

struct Input<'a>(&'a [u8]);

impl<'a> Input<'a> {
    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let (taken, rest) = self.0.split_at_checked(len)?;
        self.0 = rest;
        Some(taken)
    }

    fn number(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
}
