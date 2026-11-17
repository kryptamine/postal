use chrono::{DateTime, Local};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Paragraph, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    date_label::DateLabel, inbox_view::InboxView, message::Message, theme::Theme, truncate,
};

const MARKER: &str = "●";
/// The marker column and its gap; the subject and Snippet lines indent by the same.
const INDENT: u16 = 2;
const CARD_HEIGHT: u16 = 3;
/// Below this terminal width the date is dropped.
const MIN_WIDTH_WITH_DATE: u16 = 40;
const MIN_WIDTH: u16 = 20;

/// How many cards fit a list `height` rows tall, its border included.
pub fn capacity(height: u16) -> usize {
    usize::from(height.saturating_sub(2) / CARD_HEIGHT).max(1)
}

/// One three-line card per Message, `● sender … date` / subject / Snippet,
/// inside a border titled with the Unread count.
pub struct MessageList<'a> {
    pub theme: &'a Theme,
    pub now: DateTime<Local>,
}

impl StatefulWidget for MessageList<'_> {
    type State = InboxView;

    fn render(self, area: Rect, buf: &mut Buffer, view: &mut InboxView) {
        if area.width < MIN_WIDTH {
            return centred("terminal too small", area, buf);
        }

        let title = if view.messages.is_some() {
            format!(" Inbox  {} unread ", view.unread)
        } else {
            " Inbox ".to_owned()
        };

        let block = Block::bordered()
            .border_style(self.theme.border)
            .title(Line::styled(title, self.theme.title));
        let inner = block.inner(area);
        block.render(area, buf);
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
            now: (area.width >= MIN_WIDTH_WITH_DATE).then_some(self.now),
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
    now: Option<DateTime<Local>>,
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
        cell(buf, sender, &message.sender, strong);
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
            cell(buf, subject, &message.subject, strong);
        }
    }
}

/// Writes `text` cut to the area's width, ending in `…` when cut.
fn cell(buf: &mut Buffer, area: Rect, text: &str, style: Style) {
    let width = usize::from(area.width);
    let (shown, cut) = truncate::fit(text, width);
    let (end, _) = buf.set_stringn(area.x, area.y, shown, width, style);
    if cut {
        buf.set_string(end, area.y, "…", style);
    }
}

fn centred(text: &str, area: Rect, buf: &mut Buffer) {
    let middle = Rect::new(
        area.x,
        area.y + area.height / 2,
        area.width,
        1.min(area.height),
    );
    Paragraph::new(text)
        .alignment(Alignment::Center)
        .render(middle, buf);
}
