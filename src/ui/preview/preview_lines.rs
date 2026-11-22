use ratatui::{buffer::Buffer, layout::Rect};

use crate::{render::preview::Preview, ui::preview::link_hints::LinkHints, ui::theme::Theme};

/// The body lines of one Preview, with the labels `hints` give it as body
/// `part` of those on screen.
pub struct PreviewLines<'a> {
    pub theme: &'a Theme,
    pub preview: &'a Preview,
    pub hints: Option<&'a LinkHints>,
    pub part: usize,
}

impl PreviewLines<'_> {
    /// Body line `index` on `row`.
    pub fn draw(&self, buf: &mut Buffer, row: Rect, index: usize) {
        let (theme, preview) = (self.theme, self.preview);
        let Some(line) = preview.body.get(index) else {
            return;
        };

        let room = |x: u16| usize::from(row.right().saturating_sub(x));
        let mut x = row.x;
        for (run, segment) in line.segments().enumerate() {
            let label = self
                .hints
                .and_then(|hints| hints.label(self.part, index, run));
            // Text moves right to make room for a label; a laid out mail
            // keeps its columns, and the label covers the link's start.
            if let Some(label) = label.as_ref().filter(|_| !preview.laid_out) {
                x = buf
                    .set_stringn(x, row.y, label.as_str(), room(x), theme.label)
                    .0;
            }

            if room(x) == 0 {
                return;
            }

            let start = x;
            let style = theme.painted(segment.marks, segment.paint);
            x = buf.set_stringn(x, row.y, segment.text, room(x), style).0;
            if let Some(label) = label.filter(|_| preview.laid_out) {
                buf.set_stringn(start, row.y, label.as_str(), room(start), theme.label);
            }
        }
    }
}
