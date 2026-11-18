use crate::{
    cell_grid::{GREY_SPREAD, PAGE_LUMINANCE},
    marks::Marks,
    rgb::{BLACK, Rgb, WHITE},
};

/// Text this dark on the page is its ordinary text: the terminal's own fg.
const INK: f32 = 0.12;
/// The least contrast with what is under it, as WCAG ratios: a block must
/// stand out from the terminal, text must read, a line must show.
const BLOCK: f32 = 1.8;
const TEXT: f32 = 4.5;
const LINE: f32 = 3.0;

/// Moves mail colours, chosen for a white page, onto the terminal's
/// background, light or dark: the page goes, coloured blocks stay visible
/// and text stays readable on whatever ends up under it.
#[derive(Clone, Copy)]
pub struct MailColours {
    terminal: Rgb,
}

impl MailColours {
    pub const fn new(terminal: Rgb) -> Self {
        Self { terminal }
    }

    /// `None` where the terminal's own background shows: on the page, and on
    /// a grey `panel` too close to the terminal to stand out, which moved
    /// apart is only a slab. A control, or a colour, moves apart to show.
    pub fn background(self, bg: Option<Rgb>, panel: bool) -> Option<Rgb> {
        let bg = bg.filter(|bg| bg.luminance() <= PAGE_LUMINANCE)?;
        if panel && bg.chroma() < GREY_SPREAD && bg.contrast(self.terminal) < BLOCK {
            return None;
        }

        Some(apart(bg, self.terminal, BLOCK))
    }

    /// The fg of text over `bg`, which `background` gave; `None` for the
    /// terminal's own.
    pub fn text(self, fg: Option<Rgb>, bg: Option<Rgb>) -> Option<Rgb> {
        let fg = fg?;
        match bg {
            None if fg.luminance() < INK => None,
            None => Some(apart(fg, self.terminal, TEXT)),
            Some(bg) => Some(apart(fg, bg, TEXT)),
        }
    }

    /// A grey border on the terminal's background is drawn as Postal draws
    /// its own; a coloured one keeps its colour.
    pub fn line(self, fg: Option<Rgb>, bg: Option<Rgb>) -> (Option<Rgb>, Marks) {
        match (fg, bg) {
            (Some(fg), None) if fg.chroma() >= GREY_SPREAD => {
                (Some(apart(fg, self.terminal, LINE)), Marks::default())
            }
            (_, None) => (None, Marks::BORDER),
            (fg, Some(bg)) => (fg.map(|fg| apart(fg, bg, LINE)), Marks::default()),
        }
    }
}

/// `colour` moved toward black or white, whichever stands further from
/// `under`, until the two reach contrast `ratio`.
fn apart(colour: Rgb, under: Rgb, ratio: f32) -> Rgb {
    if colour.contrast(under) >= ratio {
        return colour;
    }

    let toward = if under.contrast(WHITE) > under.contrast(BLACK) {
        WHITE
    } else {
        BLACK
    };

    (1u8..=10)
        .map(|step: u8| colour.mix(toward, f32::from(step) / 10.0))
        .find(|moved| moved.contrast(under) >= ratio)
        .unwrap_or(toward)
}
