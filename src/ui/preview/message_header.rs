use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use unicode_width::UnicodeWidthStr;

use crate::{
    domain::message::Message, render::header_date, ui::frame::theme::Theme,
    ui::preview::attachment_line, ui::widgets::truncate,
};

/// The labels, padded so the values line up; the Subject last, which a
/// stack leaves out for its border's.
const LABELS: [&str; 3] = ["From    ", "To      ", "Subject "];
const STACKED: usize = LABELS.len() - 1;
pub const LABEL_WIDTH: u16 = 8;
/// Columns between the `From` value and the date at the row's right.
const DATE_GAP: u16 = 2;

/// The header rows over a body in the Preview, with the rule under them;
/// a Message with attachments lists them in one more.
pub fn rows(message: Option<&Message>) -> usize {
    LABELS.len() + attachment_rows(message) + 1
}

/// A stack's header rows, ending in a blank one.
pub fn stack_rows(message: &Message) -> usize {
    STACKED + attachment_rows(Some(message)) + 1
}

fn attachment_rows(message: Option<&Message>) -> usize {
    usize::from(message.is_some_and(|m| !m.attachments.is_empty()))
}

/// Each header row of `message`: its label, its value and the value's style.
pub fn fields<'a>(theme: &Theme, message: &'a Message) -> [(&'static str, &'a str, Style); 3] {
    let [from, to, subject] = LABELS;
    [
        (from, &message.from, theme.sender_of(message)),
        (to, &message.to, theme.sender),
        (subject, &message.subject, theme.subject),
    ]
}

/// Draws header row `field` of `message` in `row`, the date at the right
/// of the first. A row too narrow for both leaves the date out.
pub fn draw(theme: &Theme, buf: &mut Buffer, row: Rect, message: &Message, field: usize) {
    let Some(&(label, value, style)) = fields(theme, message).get(field) else {
        return attachments(theme, buf, row, message);
    };

    let mut row = row;
    if field == 0 {
        let date = header_date::date(&message.received);
        let width = u16::try_from(date.as_str().width()).unwrap_or(u16::MAX);
        if row.width >= LABEL_WIDTH + DATE_GAP + 2 * width {
            let x = row.right() - width;
            buf.set_string(x, row.y, date.as_str(), theme.muted);
            row.width = x - row.x - DATE_GAP;
        }
    }

    buf.set_stringn(row.x, row.y, label, usize::from(row.width), theme.field);
    let value_area = Rect {
        x: row.x + LABEL_WIDTH.min(row.width),
        width: row.width.saturating_sub(LABEL_WIDTH),
        ..row
    };

    truncate::draw(buf, value_area, value, style);
}

/// The icon in the label column, and the attachments where values go.
fn attachments(theme: &Theme, buf: &mut Buffer, row: Rect, message: &Message) {
    if message.attachments.is_empty() || row.is_empty() {
        return;
    }

    buf.set_stringn(row.x, row.y, theme.icons.attachment(), 1, theme.field);
    let value_area = Rect {
        x: row.x + LABEL_WIDTH.min(row.width),
        width: row.width.saturating_sub(LABEL_WIDTH),
        ..row
    };

    attachment_line::draw(theme, buf, value_area, &message.attachments);
}

/// Draws row `field` of `message`'s header in a stack, which has no Subject.
pub fn draw_stacked(theme: &Theme, buf: &mut Buffer, row: Rect, message: &Message, field: usize) {
    if field < STACKED {
        draw(theme, buf, row, message, field);
    } else if field == STACKED {
        attachments(theme, buf, row, message);
    }
}
