use std::fmt::Write;

use chrono::{DateTime, FixedOffset};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    centred::centred, date_label::DateLabel, inbox_view::InboxView, layout::PADDING,
    matched::Matched, message::Message, stack_str::StackStr, theme::Theme, truncate,
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

        let rows = view.rows();
        if rows == 0 {
            return centred("no matches", inner, buf);
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

        let visible = (view.offset..rows).take(fitting);
        for (y, row) in (inner.y..).step_by(CARD_HEIGHT.into()).zip(visible) {
            let index = view.search.as_ref().map_or(Some(row), |s| s.row(row));
            let Some(message) = index.and_then(|i| messages.get(i)) else {
                continue;
            };

            let matched = view.search.as_mut().zip(index).map(|(s, i)| s.matched(i));
            let card = Rect::new(inner.x, y, inner.width, CARD_HEIGHT).intersection(inner);
            cards.draw(message, row == view.selected, card, buf);
            if let Some(matched) = &matched {
                cards.highlight((message, matched), row == view.selected, card, buf);
            }
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
        let text = text_area(card);
        if message.unread {
            let style = strong.patch(tone(self.theme.marker));
            buf.set_string(text.x, text.y, MARKER, style);
        }

        let [sender, subject, snippet] = fields(card);
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

        if !subject.is_empty() {
            truncate::draw(buf, subject, &message.subject, strong);
        }

        if !snippet.is_empty() {
            truncate::draw(buf, snippet, &message.snippet, tone(self.theme.muted));
        }
    }

    /// Restyles the characters a Query matched on a drawn card.
    fn highlight(
        &self,
        (message, matched): (&Message, &Matched<'_>),
        selected: bool,
        card: Rect,
        buf: &mut Buffer,
    ) {
        let style = if selected {
            self.theme.selected.add_modifier(Modifier::UNDERLINED)
        } else {
            self.theme.matched
        };

        let texts = [&*message.sender, &*message.subject, &*message.snippet];
        let starts = matched.starts(texts);
        for ((text, start), area) in texts.into_iter().zip(starts).zip(fields(card)) {
            if !area.is_empty() {
                matched.draw(buf, area, (text, start), style);
            }
        }
    }
}

/// A card's text, padded inside the selection's fill.
const fn text_area(card: Rect) -> Rect {
    Rect {
        x: card.x + PADDING,
        width: card.width.saturating_sub(2 * PADDING),
        ..card
    }
}

/// Where a card's sender, subject and Snippet go; a row past the card is
/// empty.
fn fields(card: Rect) -> [Rect; 3] {
    let text = text_area(card);
    let text_width = text.width.saturating_sub(INDENT);
    let sender_width = (text.width / 4).clamp(12, 24).min(text_width);
    let x = text.x + INDENT;
    [
        Rect::new(x, text.y, sender_width, 1),
        Rect::new(x, text.y + 1, text_width, 1),
        Rect::new(x, text.y + 2, text_width, 1),
    ]
    .map(|field| field.intersection(card))
}
