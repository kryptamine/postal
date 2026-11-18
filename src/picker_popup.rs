use std::fmt::Write;

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    widgets::{Block, Clear, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    centred::centred, folder_picker::FolderPicker, stack_str::StackStr, theme::Theme, truncate,
};

const TITLE: &str = " Folders ";
const PROMPT: &str = "> ";
const WIDTH: u16 = 48;
/// The border, the Query row and the blank row under it.
const CHROME: u16 = 4;
/// Columns between the border and a row's text, each side.
const PADDING: u16 = 1;

/// The Folder Picker centred over `area`: the Query, then a row per Folder
/// with its Unread count right-aligned, none for `0` or one not yet counted.
pub struct PickerPopup<'a> {
    pub theme: &'a Theme,
    pub picker: &'a FolderPicker,
}

impl Widget for PickerPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = popup(area, self.picker);
        Clear.render(popup, buf);
        Block::bordered()
            .border_style(self.theme.border)
            .render(popup, buf);
        let room = usize::from(popup.width.saturating_sub(2));
        buf.set_stringn(popup.x + 1, popup.y, TITLE, room, self.theme.title);

        let inner = inner(popup);
        let (x, _) = buf.set_stringn(inner.x, inner.y, PROMPT, inner.width.into(), self.theme.key);
        let left = usize::from(inner.right().saturating_sub(x));
        buf.set_stringn(x, inner.y, self.picker.query(), left, Style::new());

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
        for ((index, folder), y) in rows.zip(list.y..) {
            let row = Rect::new(popup.x + 1, y, popup.width.saturating_sub(2), 1);
            let style = if index == self.picker.selected() {
                self.theme.selected
            } else {
                Style::new()
            };

            buf.set_style(row, style);

            let mut count = StackStr::<24>::new();
            if let Some(unread) = folder.unread.filter(|&n| n > 0) {
                let _ = write!(count, "{unread}");
            }

            let count_width = u16::try_from(count.as_str().width()).unwrap_or(0);
            let label_room = list.width.saturating_sub(count_width + 1);
            let label = Rect {
                y,
                width: label_room,
                height: 1,
                ..list
            };

            truncate::draw(buf, label, folder.label(), style);
            let count_x = list.right().saturating_sub(count_width);
            buf.set_stringn(count_x, y, count.as_str(), count_width.into(), style);
        }
    }
}

impl PickerPopup<'_> {
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

/// Tall enough for every Folder, as `area` allows.
fn popup(area: Rect, picker: &FolderPicker) -> Rect {
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
