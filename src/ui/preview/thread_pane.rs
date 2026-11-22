use std::fmt::Write;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, Clear, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    domain::message::Message,
    domain::thread,
    render::body::Body,
    render::stack_str::StackStr,
    ui::list::date_label::DateLabel,
    ui::list::inbox_view::InboxView,
    ui::preview::link_hints::LinkHints,
    ui::preview::message_header::{self, STACK_ROWS as HEADER},
    ui::preview::preview_lines::PreviewLines,
    ui::preview::preview_pane,
    ui::preview::previews::Previews,
    ui::preview::reading::Reading,
    ui::preview::stack::{self, Placed},
    ui::theme::Theme,
    ui::widgets::border,
    ui::widgets::truncate,
};

/// Columns between the sender and the Snippet, and the Snippet and the
/// date, on a folded Message.
const GAP: u16 = 2;
/// The bar left of every row of the Message at the cursor.
const CURSOR: &str = "▎";

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

            self.pin(buf, area, &placed, placed.pinned(&window));
        }

        let mut place = StackStr::<24>::new();
        write!(place, " {}/{count} ", cursor + 1).ok();
        self.title(buf, area, place.as_str());
    }
}

impl ThreadPane<'_> {
    /// Titles the border with the Thread's subject and its senders in order
    /// of first appearance, cut to fit, and `place` at the right.
    fn title(&self, buf: &mut Buffer, area: Rect, place: &str) {
        let place_width = u16::try_from(place.width()).unwrap_or(u16::MAX);
        let place_x = area.right().saturating_sub(1 + place_width).max(area.x + 1);
        border::label(buf, area, (place_x, area.y), &[place], self.theme.title);

        let stack = self.view.stack();
        let subject = stack.clone().next().map_or("", |m| m.subject.as_str());
        let first = |(n, message): &(usize, &Message)| {
            let name = thread::sender(message);
            !stack.clone().take(*n).any(|m| thread::sender(m) == name)
        };

        let names = stack.clone().enumerate().filter(first).enumerate();
        let names =
            names.flat_map(|(n, (_, m))| [if n == 0 { "" } else { ", " }, thread::sender(m)]);
        let mut x = area.x + 1;
        for part in [" ", subject, " · "].into_iter().chain(names).chain([" "]) {
            // One border cell before `place`.
            let room = Rect::new(x, area.y, place_x.saturating_sub(x + 1), 1);
            if room.is_empty() {
                break;
            }

            truncate::draw(buf, room, part, self.theme.title);
            x += u16::try_from(part.width())
                .unwrap_or(u16::MAX)
                .min(room.width);
        }
    }

    /// Row `offset` of the Message `placed`, from its top.
    fn row(&self, buf: &mut Buffer, row: Rect, placed: &Placed<'_>, offset: usize) {
        if placed.count > 1 {
            let mut earlier = StackStr::<32>::new();
            write!(earlier, "··· {} earlier messages", placed.count).ok();
            return truncate::draw(buf, row, earlier.as_str(), self.theme.muted);
        }

        let Some(body) = &placed.body else {
            return self.folded(buf, row, placed);
        };

        let Some(index) = offset.checked_sub(HEADER) else {
            // The last row of the header is blank.
            if offset + 1 < HEADER {
                message_header::draw(self.theme, buf, row, placed.message, offset);
            }

            return;
        };

        match body {
            Body::Rendered(preview) => {
                let lines = PreviewLines {
                    theme: self.theme,
                    preview,
                    hints: self.hints,
                    part: placed.index,
                };

                lines.draw(buf, row, index);
            }
            Body::Loading => truncate::draw(buf, row, "Loading…", self.theme.muted),
            Body::Unavailable(why) => truncate::draw(buf, row, why, self.theme.muted),
        }
    }

    /// Draws the last `rows` rows of the header of `placed` over the top of
    /// the text of `pane`, the last of them a rule across it as in the Preview.
    fn pin(&self, buf: &mut Buffer, pane: Rect, placed: &Placed<'_>, rows: usize) {
        let text = preview_pane::text_area(pane);
        let first = HEADER - rows;
        for (y, offset) in (text.y..text.bottom()).zip(first..HEADER) {
            let line = Rect::new(text.x, y, text.width, 1);
            Clear.render(line, buf);
            if offset + 1 == HEADER {
                border::rule(buf, pane, y, self.theme.border);
            } else {
                self.row(buf, line, placed, offset);
            }
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
