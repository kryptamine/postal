use std::fmt::Write;

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    symbols::line,
    widgets::{Block, Clear, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    domain::shown::Shown,
    render::stack_str::StackStr,
    ui::frame::theme::Theme,
    ui::popup::folder_picker::{FolderPicker, Purpose, Row},
    ui::widgets::centred::centred,
    ui::widgets::truncate,
};

const TITLE: &str = " Folders ";
const MOVING: &str = " Move to ";
const PROMPT: &str = "> ";
const WIDTH: u16 = 48;
/// The border, the Query row and the rule under it.
const CHROME: u16 = 4;
/// Columns between the border and a row's text, each side.
const PADDING: u16 = 1;

/// The Folder Picker centred over `area`: the Query, a rule, then a row per
/// Folder with its Unread count right-aligned, none for `0` or one not yet
/// counted, under each Account's address when there are several.
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
        let title = title(self.picker);
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

        if self.picker.is_empty() {
            return centred("no matches", list, buf);
        }

        let fitting = usize::from(list.height);
        let offset = (self.picker.selected() + 1).saturating_sub(fitting);
        let rows = self.picker.rows().enumerate().skip(offset).take(fitting);
        for ((index, row), y) in rows.zip(list.y..) {
            let folder = match row {
                Row::Folder(folder) => folder,
                Row::Heading(address) => {
                    let heading = Rect {
                        y,
                        height: 1,
                        ..list
                    };
                    truncate::draw(buf, heading, address, self.theme.title);
                    continue;
                }
            };

            let row = Rect::new(popup.x + 1, y, popup.width.saturating_sub(2), 1);
            let style = if index == self.picker.selected() {
                self.theme.selected
            } else {
                Style::new()
            };

            buf.set_style(row, style);
            self.folder(
                buf,
                Rect {
                    y,
                    height: 1,
                    ..list
                },
                folder,
                style,
            );
        }
    }
}

impl PickerPopup<'_> {
    /// One Folder in `row`: its icon and label, then its Unread count.
    fn folder(&self, buf: &mut Buffer, row: Rect, folder: &Shown, style: Style) {
        let mut count = StackStr::<24>::new();
        if let Some(unread) = folder.unread().filter(|&n| n > 0) {
            write!(count, "{unread}").ok();
        }

        let count_width = u16::try_from(count.as_str().width()).unwrap_or(0);
        let label = Rect {
            width: row.width.saturating_sub(count_width + 1),
            ..row
        };

        let icon = self.theme.icons.folder(folder);
        let label = icon.map_or(label, |icon| after_icon(buf, label, icon, style));
        truncate::draw(buf, label, folder.label(), style);
        let count_x = row.right().saturating_sub(count_width);
        buf.set_stringn(count_x, row.y, count.as_str(), count_width.into(), style);
    }

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

/// Draws `icon` and a gap at the start of `label`; returns what is left.
fn after_icon(buf: &mut Buffer, label: Rect, icon: &str, style: Style) -> Rect {
    let (x, _) = buf.set_stringn(label.x, label.y, icon, label.width.into(), style);
    let x = x.saturating_add(1).min(label.right());
    Rect {
        x,
        width: label.right() - x,
        ..label
    }
}

const fn title(picker: &FolderPicker) -> &'static str {
    match picker.purpose() {
        Purpose::Show => TITLE,
        Purpose::MoveTo(_) => MOVING,
    }
}

/// Tall enough for every Folder, as `area` allows.
fn popup(area: Rect, picker: &FolderPicker) -> Rect {
    let rows = u16::try_from(picker.rows().count().max(1)).unwrap_or(u16::MAX);
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
