use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use unicode_width::UnicodeWidthStr;

use crate::{
    domain::message::Message, render::header_date, ui::theme::Theme, ui::widgets::truncate,
};

/// The labels, padded so the values line up.
const LABELS: [&str; 3] = ["From    ", "To      ", "Subject "];
/// The header rows over a body, with the row under them: a rule in the
/// Preview, blank in a stack.
pub const ROWS: usize = LABELS.len() + 1;
const LABEL_WIDTH: u16 = 8;
/// Columns between the `From` value and the date at the row's right.
const DATE_GAP: u16 = 2;

/// Each header row of `message`: its label, its value and the value's style.
pub fn fields<'a>(theme: &Theme, message: &'a Message) -> [(&'static str, &'a str, Style); 3] {
    let [from, to, subject] = LABELS;
    [
        (from, &message.from, theme.sender),
        (to, &message.to, theme.sender),
        (subject, &message.subject, theme.subject),
    ]
}

/// Draws header row `field` of `message` in `row`, the date at the right
/// of the first. A row too narrow for both leaves the date out.
pub fn draw(theme: &Theme, buf: &mut Buffer, row: Rect, message: &Message, field: usize) {
    let Some(&(label, value, style)) = fields(theme, message).get(field) else {
        return;
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
