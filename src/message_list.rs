use chrono::{DateTime, Local};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Paragraph, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    date_label::DateLabel, inbox_view::InboxView, message::Message, theme::Theme, truncate,
};

const MARKER: &str = "●";
const GAP: u16 = 2;
/// Below this width the date column is dropped.
const MIN_WIDTH_WITH_DATE: u16 = 40;
const MIN_WIDTH: u16 = 20;

/// One row per Message: `● sender  subject  date`.
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
        let Some(messages) = view.messages.as_deref() else {
            return centred("Loading Inbox…", area, buf);
        };
        if messages.is_empty() {
            return centred("Inbox is empty", area, buf);
        }
        let height = usize::from(area.height);
        if view.selected < view.offset {
            view.offset = view.selected;
        } else if view.selected >= view.offset + height {
            view.offset = view.selected + 1 - height;
        }
        let visible = messages.iter().enumerate().skip(view.offset).take(height);
        for (y, (index, message)) in (area.y..).zip(visible) {
            let row = Rect::new(area.x, y, area.width, 1);
            self.row(message, index == view.selected, row, buf);
        }
    }
}

impl MessageList<'_> {
    fn row(&self, message: &Message, selected: bool, row: Rect, buf: &mut Buffer) {
        let columns = Columns::new(row.width);
        let mut style = Style::new();
        if message.unread {
            style = style.patch(self.theme.unread);
        }
        if selected {
            style = style.patch(self.theme.selected);
        }
        buf.set_style(row, style);
        if message.unread {
            buf.set_string(row.x, row.y, MARKER, style.patch(self.theme.unread_marker));
        }
        let sender_x = row.x + columns.marker;
        let sender = Rect::new(sender_x, row.y, columns.sender, 1);
        cell(buf, sender, &message.sender, style);
        let subject = Rect::new(sender_x + columns.sender + GAP, row.y, columns.subject, 1);
        cell(buf, subject, &message.subject, style);
        if columns.date > 0 {
            let label = DateLabel::new(&message.received, &self.now);
            let date = label.as_str();
            let right = row.x + row.width;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a DateLabel is at most 10 columns"
            )]
            let date_x = right - date.width() as u16;
            buf.set_string(date_x, row.y, date, style.patch(self.theme.date));
        }
    }
}

struct Columns {
    marker: u16,
    sender: u16,
    subject: u16,
    date: u16,
}

impl Columns {
    /// The sender takes 25% of the width within 12..=24 columns; the subject the rest.
    fn new(width: u16) -> Self {
        let marker = 2;
        let sender = (width / 4).clamp(12, 24);
        #[expect(clippy::cast_possible_truncation, reason = "DateLabel::WIDTH is 10")]
        let date = if width >= MIN_WIDTH_WITH_DATE {
            DateLabel::WIDTH as u16
        } else {
            0
        };
        let date_gap = if date > 0 { GAP } else { 0 };
        let subject = width.saturating_sub(marker + sender + GAP + date_gap + date);
        Self {
            marker,
            sender,
            subject,
            date,
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
