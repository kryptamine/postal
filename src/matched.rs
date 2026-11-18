use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::truncate;

/// The units of one row's haystack a Query matched, sorted. A unit is a
/// byte of an ASCII haystack, else a grapheme, as `nucleo-matcher` counts.
pub struct Matched<'a> {
    units: &'a [u32],
    ascii: bool,
}

impl<'a> Matched<'a> {
    pub const fn new(units: &'a [u32], ascii: bool) -> Self {
        Self { units, ascii }
    }

    /// Where each of `fields`, joined by one separator unit, starts.
    pub fn starts<const N: usize>(&self, fields: [&str; N]) -> [u32; N] {
        let mut next = 0;
        fields.map(|field| {
            let start = next;
            next = start + self.len(field) + 1;
            start
        })
    }

    /// Restyles the matched graphemes of `text`, drawn by `truncate::draw`
    /// on `area`'s first row; `start` is the unit `text` starts at.
    pub fn draw(&self, buf: &mut Buffer, area: Rect, (text, start): (&str, u32), style: Style) {
        if self.units.is_empty() {
            return;
        }

        let (shown, _) = truncate::fit(text, usize::from(area.width));
        let mut x = area.x;
        let mut unit = start;
        for grapheme in shown.graphemes(true) {
            let next = unit + self.step(grapheme);
            let width = u16::try_from(grapheme.width()).unwrap_or(u16::MAX);
            let from = self.units.partition_point(|&u| u < unit);
            if self.units.get(from).is_some_and(|&u| u < next) {
                buf.set_style(Rect::new(x, area.y, width, 1).intersection(area), style);
            }

            x = x.saturating_add(width);
            unit = next;
        }
    }

    fn len(&self, text: &str) -> u32 {
        text.graphemes(true).map(|g| self.step(g)).sum()
    }

    fn step(&self, grapheme: &str) -> u32 {
        if self.ascii {
            u32::try_from(grapheme.len()).unwrap_or(u32::MAX)
        } else {
            1
        }
    }
}
