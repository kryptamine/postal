use crate::{
    cell_grid::GREY_SPREAD,
    rgb::{Rgb, WHITE},
};

/// A colour this dark on a light page reads as black: WCAG luminance.
const INK: f32 = 0.12;
/// A grey this well set off from what it lies on is ordinary text even in a
/// mail that sets most of its text lighter: WCAG's enhanced contrast.
const PLAIN: f32 = 7.0;
/// Hues within this many degrees are shades of one colour.
const SHADES: f32 = 30.0;

/// What a text colour of the mail is for.
pub enum Role {
    /// The mail's ordinary text: the terminal's own fg.
    Ink,
    /// Greyer than the ordinary text: secondary, drawn muted.
    Muted,
    /// A colour, as the shade of it the mail uses most.
    Accent(Rgb),
}

/// The text colours of one mail reduced to roles, so every mail reads in
/// the terminal's ink, one muted grey and the few colours it means.
pub struct Palette {
    /// The contrast of the mail's most used grey with what it lies on.
    ink: f32,
    /// The most used shade of each colour, most used first.
    accents: Vec<Rgb>,
}

impl Palette {
    /// `texts` is each glyph's colour and the mail's colour under it.
    pub fn new(texts: impl Iterator<Item = (Rgb, Rgb)>) -> Self {
        let mut greys: Vec<(f32, usize)> = Vec::new();
        let mut colours: Vec<(Rgb, usize)> = Vec::new();
        for (fg, under) in texts {
            if grey(fg) {
                let contrast = fg.contrast(under);
                match greys.iter_mut().find(|(c, _)| (*c - contrast).abs() < 0.01) {
                    Some((_, count)) => *count += 1,
                    None => greys.push((contrast, 1)),
                }
            } else if !dark(fg, under) {
                match colours.iter_mut().find(|(c, _)| *c == fg) {
                    Some((_, count)) => *count += 1,
                    None => colours.push((fg, 1)),
                }
            }
        }

        colours.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        let mut accents: Vec<Rgb> = Vec::new();
        for (colour, _) in colours {
            if !accents.iter().any(|&accent| shades(accent, colour)) {
                accents.push(colour);
            }
        }

        let most = greys.into_iter().max_by_key(|&(_, count)| count);
        Self {
            ink: most.map_or(PLAIN, |(contrast, _)| contrast.min(PLAIN)),
            accents,
        }
    }

    /// The role of text in `fg` where the mail has `under` beneath it.
    pub fn role(&self, fg: Rgb, under: Rgb) -> Role {
        if grey(fg) {
            return if fg.contrast(under) >= self.ink * 0.95 {
                Role::Ink
            } else {
                Role::Muted
            };
        }

        if dark(fg, under) {
            return Role::Ink;
        }

        let accent = self.accents.iter().find(|&&accent| shades(accent, fg));
        Role::Accent(accent.copied().unwrap_or(fg))
    }
}

fn grey(colour: Rgb) -> bool {
    colour.chroma() < GREY_SPREAD
}

/// A colour too dark on a light page to tell from black.
fn dark(fg: Rgb, under: Rgb) -> bool {
    fg.luminance() < INK && under.contrast(WHITE) < under.contrast(fg)
}

fn shades(a: Rgb, b: Rgb) -> bool {
    let apart = (hue(a) - hue(b)).abs();
    apart.min(360.0 - apart) <= SHADES
}

/// The hue in degrees, 0 for red; a grey has none, and is never asked.
fn hue(colour: Rgb) -> f32 {
    let (r, g, b) = (
        f32::from(colour.r),
        f32::from(colour.g),
        f32::from(colour.b),
    );
    let high = r.max(g).max(b);
    let spread = high - r.min(g).min(b);
    let sixth = if high <= r {
        ((g - b) / spread).rem_euclid(6.0)
    } else if high <= g {
        (b - r) / spread + 2.0
    } else {
        (r - g) / spread + 4.0
    };

    sixth * 60.0
}
