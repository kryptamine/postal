use std::{fmt::Write, ops::Range};

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, StatefulWidget, Widget},
};

use crate::{
    border, centred::centred, inbox_view::InboxView, layout::PADDING, link_hints::LinkHints,
    preview::Preview, shown::Shown, stack_str::StackStr, theme::Theme, truncate,
};

/// The header labels, padded so the values line up; `Account` only with
/// several Accounts.
pub const LABELS: [&str; 4] = ["From    ", "Subject ", "Date    ", "Account "];
const LABEL_WIDTH: u16 = 8;

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

/// Where the body goes in a Preview's `text`: under the `header` rows,
/// which stay put while the body scrolls.
pub fn body_area(text: Rect, header: usize) -> Rect {
    let header = u16::try_from(header).unwrap_or(u16::MAX).min(text.height);
    Rect {
        y: text.y + header,
        height: text.height - header,
        ..text
    }
}

/// The body lines that `text` shows from scroll position `scroll`, under
/// `header` rows.
pub fn body_lines(scroll: usize, text: Rect, header: usize) -> Range<usize> {
    scroll..scroll.saturating_add(usize::from(body_area(text, header).height))
}

/// The selected Message's Preview inside a border titled with the Folder
/// and `i/N`: the header and a rule, then the body from the scroll position
/// on, or a placeholder in its place. In hint mode, `hints` labels its links.
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
        let rows = view.rows();
        let mut position = StackStr::<32>::new();
        (if rows > 0 {
            write!(position, " · {}/{rows} ", view.selected + 1)
        } else {
            position.write_str(" ")
        })
        .ok();

        let title = [" ", view.folder.label(), position.as_str()];
        border::label(buf, area, (area.x + 1, area.y), &title, self.theme.title);

        // No rows, as when a Search matches none, leave the pane empty.
        if rows == 0 {
            return;
        }

        let text = text_area(area);
        // An unavailable body leaves the header with no body lines under it.
        let (preview, placeholder) = match self.preview {
            Shown::Rendered(preview) => (preview, None),
            Shown::Loading => return centred("Loading…", text, buf),
            Shown::Unavailable { header, why } => (header, Some(why)),
        };

        let header_rows = view.header_rows();
        let account = view
            .selected_message()
            .and_then(|m| view.account_label(m.account));
        let values = [
            (&*preview.from, self.theme.sender),
            (&*preview.subject, self.theme.subject),
            (&*preview.date, self.theme.date),
            (account.unwrap_or_default(), self.theme.sender),
        ];

        let rows = text.y..text.bottom();
        let fields = LABELS.iter().zip(&values).take(header_rows - 1);
        for (y, (label, &(value, style))) in rows.zip(fields) {
            let row = Rect::new(text.x, y, text.width, 1);
            header(self.theme, buf, row, label, (value, style));
        }

        let body = body_area(text, header_rows);
        if usize::from(text.height) >= header_rows {
            border::rule(buf, area, body.y - 1, self.theme.border);
        }

        if let Some(placeholder) = placeholder {
            return centred(placeholder, body, buf);
        }

        let last_top = preview.body.len().saturating_sub(usize::from(body.height));
        view.preview_scroll = view.preview_scroll.min(last_top);
        let lines = view.preview_scroll..preview.body.len();
        for (y, index) in (body.y..body.bottom()).zip(lines) {
            let hints = self.hints.map(|hints| (hints, 0));
            line(
                self.theme,
                hints,
                buf,
                Rect::new(text.x, y, text.width, 1),
                (preview, index),
            );
        }
    }
}

/// One header row: `label`, then `value` cut to fit.
pub fn header(
    theme: &Theme,
    buf: &mut Buffer,
    row: Rect,
    label: &str,
    (value, style): (&str, Style),
) {
    let room = usize::from(row.width);
    buf.set_stringn(row.x, row.y, label, room, theme.field);
    let value_area = Rect {
        x: row.x + LABEL_WIDTH.min(row.width),
        width: row.width.saturating_sub(LABEL_WIDTH),
        ..row
    };

    truncate::draw(buf, value_area, value, style);
}

/// Body line `index` of `preview`, with the labels `hints` give body `part`.
pub fn line(
    theme: &Theme,
    hints: Option<(&LinkHints, usize)>,
    buf: &mut Buffer,
    row: Rect,
    (preview, index): (&Preview, usize),
) {
    let Some(line) = preview.body.get(index) else {
        return;
    };

    let room = |x: u16| usize::from(row.right().saturating_sub(x));
    let mut x = row.x;
    for (run, segment) in line.segments().enumerate() {
        let label = hints.and_then(|(hints, part)| hints.label(part, index, run));
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
