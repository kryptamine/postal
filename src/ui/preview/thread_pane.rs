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
    ui::frame::theme::Theme,
    ui::list::date_label::DateLabel,
    ui::list::inbox_view::InboxView,
    ui::preview::attachment_hints::AttachmentHints,
    ui::preview::link_hints::LinkHints,
    ui::preview::message_header,
    ui::preview::preview_lines::PreviewLines,
    ui::preview::preview_pane,
    ui::preview::previews::Previews,
    ui::preview::reading::Reading,
    ui::preview::stack::{self, Placed, Shape},
    ui::widgets::border,
    ui::widgets::truncate,
};

/// Columns between the sender and the Snippet, and the Snippet and the
/// date, on a folded Message.
const GAP: u16 = 2;
/// The bar left of every row of the Message at the cursor.
const CURSOR: &str = "▎";
/// On the bottom border while a Message marked `new` is below the window.
const BELOW: &str = " new below · G ";

/// Reading: the selected Thread's stack inside a border titled with the
/// cursor's place in it. In hint mode, `hints` labels the links, and
/// `attachment_hints` the attachments.
pub struct ThreadPane<'a> {
    pub theme: &'a Theme,
    pub view: &'a InboxView,
    pub previews: &'a Previews,
    pub hints: Option<&'a LinkHints>,
    pub attachment_hints: Option<&'a AttachmentHints>,
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
        let (mut cursor, mut below) = (0, false);
        for placed in stack::placed(self.view, self.previews, reading, text.width) {
            below |= placed.top >= window.end && reading.fresh(placed.message.body_key);
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
                self.row(buf, line, (&placed, reading), row - placed.top);
                if at_cursor && text.x > area.x + 1 {
                    buf.set_string(text.x - 1, y, CURSOR, self.theme.title);
                }
            }

            self.pin(buf, area, (&placed, reading), placed.pinned(&window));
        }

        let mut place = StackStr::<24>::new();
        write!(place, " {}/{count} ", cursor + 1).ok();
        self.title(buf, area, place.as_str());
        if below {
            let width = u16::try_from(BELOW.width()).unwrap_or(u16::MAX);
            let x = area.right().saturating_sub(1 + width).max(area.x + 1);
            border::label(
                buf,
                area,
                (x, area.bottom() - 1),
                [BELOW],
                self.theme.marker,
            );
        }
    }
}

impl ThreadPane<'_> {
    /// Titles the border with the Thread's subject and its senders in order
    /// of first appearance, cut to fit, and `place` at the right.
    fn title(&self, buf: &mut Buffer, area: Rect, place: &str) {
        let place_width = u16::try_from(place.width()).unwrap_or(u16::MAX);
        let place_x = area.right().saturating_sub(1 + place_width).max(area.x + 1);
        border::label(buf, area, (place_x, area.y), [place], self.theme.title);

        let stack = self.view.stack();
        let subject = stack.clone().next().map_or("", |m| m.subject.as_str());
        let first = |(n, message): &(usize, &Message)| {
            let name = thread::sender(message);
            !stack.clone().take(*n).any(|m| thread::sender(m) == name)
        };

        let names = stack.clone().enumerate().filter(first).enumerate();
        let names =
            names.flat_map(|(n, (_, m))| [if n == 0 { "" } else { ", " }, thread::sender(m)]);
        let parts = [" ", subject, " · "].into_iter().chain(names).chain([" "]);
        // Short of `place`: the label stops one cell before its pane's right.
        let left = Rect {
            width: place_x - area.x,
            ..area
        };

        border::label(buf, left, (area.x + 1, area.y), parts, self.theme.title);
    }

    /// Row `offset` of the Message `placed`, from its top.
    fn row(
        &self,
        buf: &mut Buffer,
        row: Rect,
        (placed, reading): (&Placed<'_>, &Reading),
        offset: usize,
    ) {
        let body = match &placed.shape {
            Shape::Run(count) => {
                let mut earlier = StackStr::<32>::new();
                write!(earlier, "··· {count} earlier messages").ok();
                return truncate::draw(buf, row, earlier.as_str(), self.theme.muted);
            }
            Shape::Folded => return self.folded(buf, row, placed),
            Shape::Unfolded(body) => body,
        };

        let Some(index) = offset.checked_sub(placed.header()) else {
            let message = placed.message;
            let labels = self.attachment_hints.and_then(|h| h.labels(message));
            let fresh = reading.fresh(message.body_key);
            return message_header::draw_stacked(
                self.theme,
                buf,
                row,
                (message, offset),
                (labels, fresh),
            );
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
    fn pin(
        &self,
        buf: &mut Buffer,
        pane: Rect,
        (placed, reading): (&Placed<'_>, &Reading),
        rows: usize,
    ) {
        let text = preview_pane::text_area(pane);
        let header = placed.header();
        let first = header - rows;
        for (y, offset) in (text.y..text.bottom()).zip(first..header) {
            let line = Rect::new(text.x, y, text.width, 1);
            Clear.render(line, buf);
            if offset + 1 == header {
                border::rule(buf, pane, y, self.theme.border);
            } else {
                self.row(buf, line, (placed, reading), offset);
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
        let style = self.theme.sender_of(message);
        let style = if message.unread {
            style.patch(self.theme.unread)
        } else {
            style
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
