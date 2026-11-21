use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    symbols::line,
    widgets::{Block, Clear, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{centred::centred, recipient_picker::RecipientPicker, theme::Theme, truncate};

const PROMPT: &str = "> ";
const WIDTH: u16 = 64;
/// The border, the Query row and the rule under it.
const CHROME: u16 = 4;
/// Columns between the border and a row's text, each side.
const PADDING: u16 = 1;

/// The Correspondents a recipient could expand to, centred over `area`:
/// titled with the recipient as written, then the Query, a rule and a row
/// each, with the Account that knows it right-aligned when there are several.
pub struct RecipientPopup<'a> {
    pub theme: &'a Theme,
    pub picker: &'a RecipientPicker,
    pub accounts: bool,
}

impl Widget for RecipientPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = popup(area, self.picker);
        Clear.render(popup, buf);
        Block::bordered()
            .border_style(self.theme.border)
            .render(popup, buf);
        let room = usize::from(popup.width.saturating_sub(2));
        let title = format!(" {} → ", self.picker.entry);
        buf.set_stringn(popup.x + 1, popup.y, title, room, self.theme.title);

        let inner = inner(popup);
        let (x, _) = buf.set_stringn(inner.x, inner.y, PROMPT, inner.width.into(), self.theme.key);
        let left = usize::from(inner.right().saturating_sub(x));
        buf.set_stringn(x, inner.y, self.picker.query(), left, Style::new());

        let rule = Rect::new(popup.x, inner.y + 1, popup.width, 1);
        if rule.width >= 2 && rule.y < popup.bottom().saturating_sub(1) {
            let middle = line::HORIZONTAL.repeat(usize::from(rule.width.saturating_sub(2)));
            let joined = format!("{}{middle}{}", line::VERTICAL_RIGHT, line::VERTICAL_LEFT);
            buf.set_string(rule.x, rule.y, joined, self.theme.border);
        }

        let list = Rect {
            y: inner.y + 2,
            height: inner.height.saturating_sub(2),
            ..inner
        };

        if self.picker.rows().len() == 0 {
            return centred("no matches", list, buf);
        }

        let fitting = usize::from(list.height);
        let offset = (self.picker.selected() + 1).saturating_sub(fitting);
        let rows = self.picker.rows().enumerate().skip(offset).take(fitting);
        for ((index, correspondent), y) in rows.zip(list.y..) {
            let style = if index == self.picker.selected() {
                self.theme.selected
            } else {
                Style::new()
            };

            let row = Rect::new(popup.x + 1, y, popup.width.saturating_sub(2), 1);
            buf.set_style(row, style);
            let row = Rect {
                y,
                height: 1,
                ..list
            };

            let account = if self.accounts {
                correspondent.account.as_str()
            } else {
                ""
            };

            let account_width = u16::try_from(account.width()).unwrap_or(0);
            let label = Rect {
                width: row.width.saturating_sub(account_width + 1),
                ..row
            };

            truncate::draw(buf, label, &correspondent.mailbox(), style);
            let account_x = row.right().saturating_sub(account_width);
            let style = style.patch(self.theme.field);
            buf.set_stringn(account_x, y, account, account_width.into(), style);
        }
    }
}

impl RecipientPopup<'_> {
    /// Where the terminal's cursor goes: after the Query.
    pub fn cursor(&self, area: Rect) -> Position {
        let inner = inner(popup(area, self.picker));
        let typed = PROMPT.width() + self.picker.query().width();
        let x = inner
            .x
            .saturating_add(u16::try_from(typed).unwrap_or(u16::MAX))
            .min(inner.right().saturating_sub(1));
        Position::new(x, inner.y)
    }
}

/// Tall enough for every match, as `area` allows.
fn popup(area: Rect, picker: &RecipientPicker) -> Rect {
    let rows = u16::try_from(picker.rows().len().max(1)).unwrap_or(u16::MAX);
    let width = WIDTH.min(area.width);
    let height = rows.saturating_add(CHROME).min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

const fn inner(popup: Rect) -> Rect {
    Rect {
        x: popup.x.saturating_add(1 + PADDING),
        y: popup.y.saturating_add(1),
        width: popup.width.saturating_sub(2 + 2 * PADDING),
        height: popup.height.saturating_sub(2),
    }
}
