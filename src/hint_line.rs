use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};
use tokio::time::Instant;
use unicode_width::UnicodeWidthStr;

use crate::{
    keymap::{BINDINGS, HELP_HINT, Hint},
    status::Status,
    theme::Theme,
};

const GAP: u16 = 2;

/// The bottom row: key hints on the left, a Toast or else the Status on the
/// right. Hints that do not fit drop from the right; `?: help` always stays.
pub struct HintLine<'a> {
    pub theme: &'a Theme,
    pub toast: Option<&'static str>,
    pub status: Status,
    /// What the Status's countdown counts from.
    pub now: Instant,
}

impl Widget for HintLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let row = Rect {
            x: area.x.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.min(1),
            ..area
        };

        if row.is_empty() {
            return;
        }

        let status = self.status.text(self.now);
        let (right, style) = match self.toast {
            Some(text) => (text, Style::new()),
            None if self.status.warns() => (status.as_str(), self.theme.warning),
            None => (status.as_str(), self.theme.muted),
        };

        let reserved = if right.is_empty() {
            0
        } else {
            right.width() + usize::from(GAP)
        };

        let room = usize::from(row.width).saturating_sub(reserved);
        let mut left = room.saturating_sub(width(&HELP_HINT));
        let mut x = row.x;
        for hint in BINDINGS.iter().filter_map(|b| b.hint.as_ref()) {
            let needed = width(hint) + usize::from(GAP);
            if needed > left {
                break;
            }

            left -= needed;
            x = self.hint(buf, x, row, hint).saturating_add(GAP);
        }

        let end = self.hint(buf, x, row, &HELP_HINT);
        if !right.is_empty() {
            let width = u16::try_from(right.width()).unwrap_or(u16::MAX);
            let at = row
                .right()
                .saturating_sub(width)
                .max(end.saturating_add(GAP))
                .min(row.right());
            let room = usize::from(row.right() - at);
            buf.set_stringn(at, row.y, right, room, style);
        }
    }
}

impl HintLine<'_> {
    /// Draws `hint` from `x`, cut at the row's end; returns where it ends.
    fn hint(&self, buf: &mut Buffer, x: u16, row: Rect, hint: &Hint) -> u16 {
        let room = |x: u16| usize::from(row.right().saturating_sub(x));
        let (x, _) = buf.set_stringn(x, row.y, hint.keys, room(x), self.theme.key);
        let (x, _) = buf.set_stringn(x, row.y, " ", room(x), self.theme.muted);
        buf.set_stringn(x, row.y, hint.description, room(x), self.theme.muted)
            .0
    }
}

fn width(hint: &Hint) -> usize {
    hint.keys.width() + 1 + hint.description.width()
}
