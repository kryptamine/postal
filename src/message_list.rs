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
    centred::centred, date_label::DateLabel, inbox_view::InboxView, layout::PADDING,
    message::Message, stack_str::StackStr, theme::Theme, truncate,
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

        // The selected card keeps one style throughout: nothing on it is
        // muted or coloured, which would break the selection's colour.
        let tone = |style: Style| if selected { base } else { base.patch(style) };
        let strong = if message.unread {
            base.patch(self.theme.unread)
        } else {
            base
        };

        // The selection fills the whole card; only its text is padded.
        buf.set_style(card, base);
        let text = Rect {
            x: card.x + PADDING,
            width: card.width.saturating_sub(2 * PADDING),
            ..card
        };

        if message.unread {
            let style = strong.patch(tone(self.theme.marker));
            buf.set_string(text.x, text.y, MARKER, style);
        }

        let text_width = text.width.saturating_sub(INDENT);
        let sender_width = (text.width / 4).clamp(12, 24).min(text_width);
        let sender = Rect::new(text.x + INDENT, text.y, sender_width, 1);
        let style = strong.patch(tone(self.theme.sender));
        truncate::draw(buf, sender, &message.sender, style);
        if let Some(now) = &self.now {
            let label = DateLabel::new(&message.received, now);
            let date = label.as_str();
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a DateLabel is at most 10 columns"
            )]
            let date_x = text.right() - date.width() as u16;
            buf.set_string(date_x, text.y, date, tone(self.theme.date));
        }

        if text.height > 1 {
            let subject = Rect::new(text.x + INDENT, text.y + 1, text_width, 1);
            truncate::draw(buf, subject, &message.subject, strong);
        }

        if text.height > 2 {
            let snippet = Rect::new(text.x + INDENT, text.y + 2, text_width, 1);
            truncate::draw(buf, snippet, &message.snippet, tone(self.theme.muted));
        }
    }
}
