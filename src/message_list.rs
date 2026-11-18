use std::fmt::Write;

use chrono::{DateTime, FixedOffset};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    centred::centred, date_label::DateLabel, inbox_view::InboxView, message::Message,
    stack_str::StackStr, theme::Theme, truncate,
};

const MARKER: &str = "●";
/// The marker column and its gap; the subject and Snippet lines indent by the same.
const INDENT: u16 = 2;
const CARD_HEIGHT: u16 = 3;
/// Below this list width the date is dropped. The list beside the Preview
/// is never narrower than this.
const MIN_WIDTH_WITH_DATE: u16 = 29;
pub const MIN_WIDTH: u16 = 20;

/// How many cards fit a list `height` rows tall, its border included.
pub fn capacity(height: u16) -> usize {
    usize::from(height.saturating_sub(2) / CARD_HEIGHT).max(1)
}

/// One three-line card per Message, `● sender … date` / subject / Snippet,
/// inside a border titled with the Unread count.
pub struct MessageList<'a> {
    pub theme: &'a Theme,
}

impl StatefulWidget for MessageList<'_> {
    type State = InboxView;

    fn render(self, area: Rect, buf: &mut Buffer, view: &mut InboxView) {
        if area.width < MIN_WIDTH {
            return centred("terminal too small", area, buf);
        }

        let block = Block::bordered().border_style(self.theme.border);
        let inner = block.inner(area);
        block.render(area, buf);
        // Drawn over the border rather than as a Block title, which allocates.
        let mut title = StackStr::<40>::new();
        let _ = if view.messages.is_some() {
            write!(title, " Inbox  {} unread ", view.unread)
        } else {
            title.write_str(" Inbox ")
        };

        let title_width = usize::from(inner.width);
        buf.set_stringn(
            inner.x,
            area.y,
            title.as_str(),
            title_width,
            self.theme.title,
        );
        let Some(messages) = view.messages.as_deref() else {
            return centred("Loading Inbox…", inner, buf);
        };

        if messages.is_empty() {
            return centred("Inbox is empty", inner, buf);
        }

        let fitting = capacity(area.height);
        if view.selected < view.offset {
            view.offset = view.selected;
        } else if view.selected >= view.offset + fitting {
            view.offset = view.selected + 1 - fitting;
        }

        let cards = Cards {
            theme: self.theme,
            now: (area.width >= MIN_WIDTH_WITH_DATE).then(|| view.now()),
        };

        let visible = messages.iter().enumerate().skip(view.offset).take(fitting);
        for (y, (index, message)) in (inner.y..).step_by(CARD_HEIGHT.into()).zip(visible) {
            let card = Rect::new(inner.x, y, inner.width, CARD_HEIGHT).intersection(inner);
            cards.draw(message, index == view.selected, card, buf);
        }
    }
}

/// Draws cards; `now` is `None` when the terminal is too narrow for the date.
struct Cards<'a> {
    theme: &'a Theme,
    now: Option<DateTime<FixedOffset>>,
}

impl Cards<'_> {
    fn draw(&self, message: &Message, selected: bool, card: Rect, buf: &mut Buffer) {
        let base = if selected {
            self.theme.selected
        } else {
            Style::new()
        };

        // The selected card keeps one style throughout, so nothing on it is muted.
        let muted = if selected {
            base
        } else {
            base.patch(self.theme.muted)
        };

        let strong = if message.unread {
            base.patch(self.theme.unread)
        } else {
            base
        };

        buf.set_style(card, base);
        if message.unread {
            buf.set_string(card.x, card.y, MARKER, strong);
        }

        let text_width = card.width.saturating_sub(INDENT);
        let sender_width = (card.width / 4).clamp(12, 24).min(text_width);
        let sender = Rect::new(card.x + INDENT, card.y, sender_width, 1);
        truncate::draw(buf, sender, &message.sender, strong);
        if let Some(now) = &self.now {
            let label = DateLabel::new(&message.received, now);
            let date = label.as_str();
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a DateLabel is at most 10 columns"
            )]
            let date_x = card.right() - date.width() as u16;
            buf.set_string(date_x, card.y, date, muted);
        }

        if card.height > 1 {
            let subject = Rect::new(card.x + INDENT, card.y + 1, text_width, 1);
            truncate::draw(buf, subject, &message.subject, strong);
        }

        if card.height > 2 {
            let snippet = Rect::new(card.x + INDENT, card.y + 2, text_width, 1);
            truncate::draw(buf, snippet, &message.snippet, muted);
        }
    }
}
