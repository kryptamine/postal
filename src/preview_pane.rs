use std::fmt::Write;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, StatefulWidget, Widget},
};

use crate::{
    centred::centred, inbox_view::InboxView, preview::Preview, stack_str::StackStr,
    styled_line::StyledLine, theme::Theme, truncate,
};

/// The header labels, padded so the values line up.
pub const LABELS: [&str; 3] = ["From    ", "Subject ", "Date    "];
const LABEL_WIDTH: u16 = 8;
/// The header rows and the blank row under them.
const HEADER_ROWS: usize = LABELS.len() + 1;

/// Where a Preview's text goes in `pane`: inside the border, with one column
/// of padding each side and none on top.
pub const fn text_area(pane: Rect) -> Rect {
    Rect {
        x: pane.x.saturating_add(2),
        y: pane.y.saturating_add(1),
        width: pane.width.saturating_sub(4),
        height: pane.height.saturating_sub(2),
    }
}

/// The selected Message's Preview inside a border titled `i/N`: the header,
/// then the body from the scroll position on. `preview` is `None` until it
/// is rendered.
pub struct PreviewPane<'a> {
    pub theme: &'a Theme,
    pub preview: Option<&'a Preview>,
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
        let Some(preview) = self.preview else {
            return centred("Loading…", text, buf);
        };

        let rows = HEADER_ROWS + preview.body.len();
        let last_top = rows.saturating_sub(usize::from(text.height));
        view.preview_scroll = view.preview_scroll.min(last_top);
        let header = [
            (&preview.from, Style::new()),
            (&preview.subject, self.theme.title),
            (&preview.date, Style::new()),
        ];

        for (y, index) in (text.y..text.bottom()).zip(view.preview_scroll..rows) {
            let row = Rect::new(text.x, y, text.width, 1);
            if let Some(&(value, style)) = header.get(index) {
                self.header(buf, row, LABELS[index], (value, style));
            } else if let Some(line) = preview.body.get(index.wrapping_sub(HEADER_ROWS)) {
                self.line(buf, row, line);
            }
        }
    }
}

impl PreviewPane<'_> {
    fn header(&self, buf: &mut Buffer, row: Rect, label: &str, (value, style): (&str, Style)) {
        let room = usize::from(row.width);
        buf.set_stringn(row.x, row.y, label, room, self.theme.muted);
        let value_area = Rect {
            x: row.x + LABEL_WIDTH.min(row.width),
            width: row.width.saturating_sub(LABEL_WIDTH),
            ..row
        };

        truncate::draw(buf, value_area, value, style);
    }

    fn line(&self, buf: &mut Buffer, row: Rect, line: &StyledLine) {
        let mut x = row.x;
        for (text, marks) in line.segments() {
            let room = usize::from(row.right().saturating_sub(x));
            if room == 0 {
                return;
            }

            x = buf
                .set_stringn(x, row.y, text, room, self.theme.marked(marks))
                .0;
        }
    }
}
