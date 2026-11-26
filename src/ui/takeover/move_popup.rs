use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, Clear, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::ui::{frame::theme::Theme, widgets::popup_area::popup_area};

const TITLE: &str = " Postal ";
/// The border and one column or row of padding inside it, each side.
const FRAME: u16 = 4;

/// What a takeover says, centred over the screen: its lines, then the keys
/// it answers to.
pub struct MovePopup<'a> {
    pub theme: &'a Theme,
    pub lines: &'a [&'a str],
    pub keys: &'a str,
}

impl Widget for MovePopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let widest = (self.lines.iter().copied())
            .chain([self.keys])
            .map(UnicodeWidthStr::width)
            .max()
            .unwrap_or(0);
        let width = u16::try_from(widest).unwrap_or(u16::MAX) + FRAME;
        let rows = u16::try_from(self.lines.len()).unwrap_or(u16::MAX);
        // A blank row between the lines and the keys.
        let popup = popup_area(area, width, rows + 2 + FRAME);
        Clear.render(popup, buf);
        Block::bordered()
            .border_style(self.theme.border)
            .render(popup, buf);
        let room = usize::from(popup.width.saturating_sub(FRAME));
        buf.set_stringn(popup.x + 1, popup.y, TITLE, room, self.theme.title);

        let (x, mut y) = (popup.x + 2, popup.y + 2);
        let bottom = popup.bottom().saturating_sub(1);
        for line in self.lines {
            if y >= bottom {
                return;
            }

            buf.set_stringn(x, y, line, room, self.theme.subject);
            y += 1;
        }

        if y + 1 < bottom {
            buf.set_stringn(x, y + 1, self.keys, room, self.theme.muted);
        }
    }
}
