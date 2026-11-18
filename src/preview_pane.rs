use std::{fmt::Write, ops::Range};

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, StatefulWidget, Widget},
};

use crate::{
    centred::centred, inbox_view::InboxView, layout::PADDING, link_hints::LinkHints,
    preview::Preview, shown::Shown, stack_str::StackStr, theme::Theme, truncate,
};

/// The header labels, padded so the values line up.
pub const LABELS: [&str; 3] = ["From    ", "Subject ", "Date    "];
const LABEL_WIDTH: u16 = 8;
/// The header rows and the blank row under them.
const HEADER_ROWS: usize = LABELS.len() + 1;

/// Where a Preview's text goes in `pane`: inside the border, padded each
/// side and not on top.
pub const fn text_area(pane: Rect) -> Rect {
    Rect {
        x: pane.x.saturating_add(1 + PADDING),
        y: pane.y.saturating_add(1),
        width: pane.width.saturating_sub(2 * (1 + PADDING)),
        height: pane.height.saturating_sub(2),
    }
}

/// The body lines that `text` shows from scroll position `scroll`.
pub const fn body_lines(scroll: usize, text: Rect) -> Range<usize> {
    let end = scroll + text.height as usize;
    scroll.saturating_sub(HEADER_ROWS)..end.saturating_sub(HEADER_ROWS)
}

/// The selected Message's Preview inside a border titled `i/N`: the header,
/// then the body from the scroll position on, or a placeholder in its
/// place. In hint mode, `hints` labels its links.
pub struct PreviewPane<'a> {
    pub theme: &'a Theme,
    pub preview: Shown<'a>,
    pub hints: Option<&'a LinkHints>,
}

impl StatefulWidget for PreviewPane<'_> {
    type State = InboxView;

    fn render(self, area: Rect, buf: &mut Buffer, view: &mut InboxView) {
        Block::bordered()
            .border_style(self.theme.border)
            .render(area, buf);
        let Some(count) = view.messages.as_ref().map(Vec::len).filter(|&n| n > 0) else {
            return;
        };

        // Drawn over the border rather than as a Block title, which allocates.
        let mut title = StackStr::<24>::new();
        let _ = write!(title, " {}/{count} ", view.selected + 1);
        let room = usize::from(area.width.saturating_sub(2));
        buf.set_stringn(area.x + 1, area.y, title.as_str(), room, self.theme.title);
        let text = text_area(area);
        // An unavailable body leaves the header with no body lines under it.
        let (preview, placeholder) = match self.preview {
            Shown::Rendered(preview) => (preview, None),
            Shown::Loading => return centred("Loading…", text, buf),
            Shown::Unavailable { header, why } => (header, Some(why)),
        };

        let rows = HEADER_ROWS + preview.body.len();
        let last_top = rows.saturating_sub(usize::from(text.height));
        view.preview_scroll = view.preview_scroll.min(last_top);
        let header = [
            (&preview.from, self.theme.sender),
            (&preview.subject, self.theme.subject),
            (&preview.date, self.theme.date),
        ];

        for (y, index) in (text.y..text.bottom()).zip(view.preview_scroll..rows) {
            let row = Rect::new(text.x, y, text.width, 1);
            if let Some(&(value, style)) = header.get(index) {
                self.header(buf, row, LABELS[index], (value, style));
            } else if let Some(line) = index.checked_sub(HEADER_ROWS) {
                self.line(buf, row, preview, line);
            }
        }

        if let Some(placeholder) = placeholder {
            let header = u16::try_from(HEADER_ROWS).unwrap_or(u16::MAX);
            let top = text.y.saturating_add(header).min(text.bottom());
            let below = Rect {
                y: top,
                height: text.bottom() - top,
                ..text
            };

            centred(placeholder, below, buf);
        }
    }
}

impl PreviewPane<'_> {
    fn header(&self, buf: &mut Buffer, row: Rect, label: &str, (value, style): (&str, Style)) {
        let room = usize::from(row.width);
        buf.set_stringn(row.x, row.y, label, room, self.theme.field);
        let value_area = Rect {
            x: row.x + LABEL_WIDTH.min(row.width),
            width: row.width.saturating_sub(LABEL_WIDTH),
            ..row
        };

        truncate::draw(buf, value_area, value, style);
    }

    fn line(&self, buf: &mut Buffer, row: Rect, preview: &Preview, index: usize) {
        let Some(line) = preview.body.get(index) else {
            return;
        };

        let room = |x: u16| usize::from(row.right().saturating_sub(x));
        let mut x = row.x;
        for (run, segment) in line.segments().enumerate() {
            // Drawn before the link text, which moves right to make room.
            if let Some(label) = self.hints.and_then(|hints| hints.label(index, run)) {
                x = buf
                    .set_stringn(x, row.y, label.as_str(), room(x), self.theme.label)
                    .0;
            }

            if room(x) == 0 {
                return;
            }

            let style = self.theme.marked(segment.marks);
            x = buf.set_stringn(x, row.y, segment.text, room(x), style).0;
        }
    }
}
