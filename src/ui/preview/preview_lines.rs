use ratatui::{buffer::Buffer, layout::Rect};
use unicode_width::UnicodeWidthStr;

use crate::{
    render::preview::Preview, ui::frame::theme::Theme, ui::preview::link_hints::LinkHints,
};

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
            // keeps its columns, and the label goes before or over the link.
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
                let at = label_at(buf, row, start, label.as_str().width());
                buf.set_stringn(at, row.y, label.as_str(), room(at), theme.label);
            }
        }
    }
}

/// Where a laid out link's label of `width` goes: in the blank cells the line
/// already drew before the link at `start`, or else over the link's start.
fn label_at(buf: &Buffer, row: Rect, start: u16, width: usize) -> u16 {
    let blank = |x: u16| {
        buf.cell((x, row.y))
            .is_some_and(|cell| cell.symbol() == " ")
    };
    u16::try_from(width)
        .ok()
        .and_then(|width| start.checked_sub(width))
        .filter(|&before| before >= row.x && (before..start).all(blank))
        .unwrap_or(start)
}
