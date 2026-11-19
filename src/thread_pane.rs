use std::fmt::Write;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    body::Body,
    border,
    date_label::DateLabel,
    inbox_view::InboxView,
    link_hints::LinkHints,
    preview_pane::{self, HEADER_ROWS, LABELS},
    preview_render,
    previews::Previews,
    reading::Reading,
    stack::{self, Placed},
    stack_str::StackStr,
    theme::Theme,
    thread, truncate,
};

/// Columns between the sender and the Snippet, and the Snippet and the
/// date, on a folded Message.
const GAP: u16 = 2;
/// The bar left of every row of the Message at the cursor.
const CURSOR: &str = "▎";
/// The header row of `LABELS` that holds the date.
const DATE_ROW: usize = 2;

/// Reading: the selected Thread's stack inside a border titled with the
/// cursor's place in it. In hint mode, `hints` labels the links.
pub struct ThreadPane<'a> {
    pub theme: &'a Theme,
    pub view: &'a InboxView,
    pub previews: &'a Previews,
    pub hints: Option<&'a LinkHints>,
}

impl StatefulWidget for ThreadPane<'_> {
    type State = Reading;

    fn render(self, area: Rect, buf: &mut Buffer, reading: &mut Reading) {
        Block::bordered()
            .border_style(self.theme.border)
            .render(area, buf);
        let text = preview_pane::text_area(area);
        let height = usize::from(text.height);
        stack::settle(self.view, self.previews, reading, (text.width, height));
        let window = reading.scroll..reading.scroll + height;
        let count = self.view.stack().count();
        let mut cursor = 0;
        for placed in stack::placed(self.view, self.previews, reading, text.width) {
            // A Thread of one has nothing else to point at.
            let at_cursor = placed.message.body_key == reading.cursor && count > 1;
            if at_cursor {
                cursor = placed.index;
            }

            if !placed.shown(&window) {
                continue;
            }

            let rows = placed.top.max(window.start)..placed.bottom().min(window.end);
            for row in rows {
                let y = text.y + u16::try_from(row - window.start).unwrap_or(u16::MAX);
                let line = Rect::new(text.x, y, text.width, 1);
                self.row(buf, line, &placed, row - placed.top);
                if at_cursor && text.x > area.x + 1 {
                    buf.set_string(text.x - 1, y, CURSOR, self.theme.title);
                }
            }
        }

        let mut title = StackStr::<24>::new();
        write!(title, " {}/{count} ", cursor + 1).ok();
        let title = [title.as_str()];
        border::label(buf, area, (area.x + 1, area.y), &title, self.theme.title);
    }
}

impl ThreadPane<'_> {
    /// Row `offset` of the Message `placed`, from its top.
    fn row(&self, buf: &mut Buffer, row: Rect, placed: &Placed<'_>, offset: usize) {
        let message = placed.message;
        let Some(body) = &placed.body else {
            return self.folded(buf, row, placed);
        };

        // From the row, not the Preview, so the header stays put while the
        // body loads.
        let mut date = StackStr::<40>::new();
        if offset == DATE_ROW {
            write!(date, "{}", message.received.format(preview_render::DATE)).ok();
        }

        let values = [
            (message.sender.as_str(), self.theme.sender),
            (message.subject.as_str(), self.theme.subject),
            (date.as_str(), self.theme.date),
        ];

        if let Some(&value) = values.get(offset) {
            return preview_pane::header(self.theme, buf, row, LABELS[offset], value);
        }

        let Some(index) = offset.checked_sub(HEADER_ROWS) else {
            return;
        };

        match body {
            Body::Rendered(preview) => {
                let hints = self.hints.map(|hints| (hints, placed.index));
                preview_pane::line(self.theme, hints, buf, row, (preview, index));
            }
            Body::Loading => truncate::draw(buf, row, "Loading…", self.theme.muted),
            Body::Unavailable(why) => truncate::draw(buf, row, why, self.theme.muted),
        }
    }

    /// The one line of a folded Message: its sender, its Snippet, its date.
    fn folded(&self, buf: &mut Buffer, row: Rect, placed: &Placed<'_>) {
        let message = placed.message;
        let label = DateLabel::new(&message.received, &self.view.now());
        let date = label.as_str();
        let date_width = u16::try_from(date.width()).unwrap_or(u16::MAX);
        let date_x = row.right().saturating_sub(date_width);
        buf.set_stringn(
            date_x,
            row.y,
            date,
            usize::from(date_width),
            self.theme.date,
        );

        let sender = thread::sender(message);
        let style = if message.unread {
            self.theme.sender.patch(self.theme.unread)
        } else {
            self.theme.sender
        };

        let room = date_x.saturating_sub(row.x).saturating_sub(GAP);
        let sender_width = u16::try_from(sender.width()).unwrap_or(u16::MAX).min(room);
        let snippet_x = row.x + sender_width + GAP;
        let snippet = Rect {
            x: snippet_x,
            width: date_x.saturating_sub(GAP).saturating_sub(snippet_x),
            ..row
        };

        // An empty area would still take the `…` of a cut.
        for (area, text, style) in [
            (
                Rect {
                    width: sender_width,
                    ..row
                },
                sender,
                style,
            ),
            (snippet, message.snippet.as_str(), self.theme.muted),
        ] {
            if !area.is_empty() {
                truncate::draw(buf, area, text, style);
            }
        }
    }
}
