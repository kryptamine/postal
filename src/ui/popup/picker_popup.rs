use std::fmt::Write;

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    symbols::line,
    widgets::{Block, Clear, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    domain::counts::Counts,
    domain::shown::Shown,
    render::stack_str::StackStr,
    ui::frame::theme::Theme,
    ui::popup::folder_picker::{FolderPicker, Purpose, Row},
    ui::widgets::centred::centred,
    ui::widgets::{input, popup_area::popup_area, scrollbar::scrollbar, truncate},
};

const TITLE: &str = " Folders ";
const MOVING: &str = " Move to ";
const PROMPT: &str = "> ";
const WIDTH: u16 = 48;
/// The border, the Query row and the rule under it.
const CHROME: u16 = 4;
/// Columns between the border and a row's text, each side.
const PADDING: u16 = 1;
/// The column the scrollbar takes, inside the right padding.
const BAR: u16 = 1;

/// The Folder Picker centred over `area`: the Query, a rule, then a row per
/// Folder with its counts right-aligned, under each Account's address when
/// there are several; scrolled to keep the selected row in view. Its state is
/// where the terminal's cursor goes: after the Query.
pub struct PickerPopup<'a> {
    pub theme: &'a Theme,
    pub picker: &'a FolderPicker,
}

impl StatefulWidget for PickerPopup<'_> {
    type State = Option<Position>;

    fn render(self, area: Rect, buf: &mut Buffer, cursor: &mut Option<Position>) {
        let popup = popup(area, self.picker);
        Clear.render(popup, buf);
        Block::bordered()
            .border_style(self.theme.border)
            .render(popup, buf);
        let room = usize::from(popup.width.saturating_sub(2));
        let title = title(self.picker);
        buf.set_stringn(popup.x + 1, popup.y, title, room, self.theme.title);

        if popup.height < 3 {
            return;
        }

        let inner = inner(popup);
        *cursor = self.query(buf, popup, inner);
        let list = Rect {
            y: inner.y + 2,
            height: inner.height.saturating_sub(2),
            ..inner
        };

        if self.picker.is_empty() {
            return centred("no matches", list, buf);
        }

        self.draw_rows(buf, popup, list);
    }
}

impl PickerPopup<'_> {
    fn draw_rows(&self, buf: &mut Buffer, popup: Rect, list: Rect) {
        let fitting = usize::from(list.height);
        let total = self.picker.rows().count();
        let scrollable = total > fitting;
        let offset = (self.picker.selected() + 1).saturating_sub(fitting);
        let rows = self.picker.rows().enumerate().skip(offset).take(fitting);
        let highlight_width = popup
            .width
            .saturating_sub(2 + if scrollable { BAR } else { 0 });
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

            let row = Rect::new(popup.x + 1, y, highlight_width, 1);
            let style = if index == self.picker.selected() {
                self.theme.chosen
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

        // Leave a blank column between right-aligned counts and the thumb.
        let track = Rect::new(popup.right().saturating_sub(2), list.y, BAR, list.height);
        scrollbar(buf, track, (offset, total), self.theme.muted);
    }

    /// The Query on the first row of `inner`, and the rule under it; returns
    /// where the cursor goes.
    fn query(&self, buf: &mut Buffer, popup: Rect, inner: Rect) -> Option<Position> {
        let (x, _) = buf.set_stringn(inner.x, inner.y, PROMPT, inner.width.into(), self.theme.key);
        let typed = Rect::new(x, inner.y, inner.right().saturating_sub(x), 1);
        let cursor = input::draw(buf, typed, self.picker.query(), Style::new());

        let rule = Rect::new(popup.x, inner.y + 1, popup.width, 1);
        if rule.width >= 2 && rule.y < popup.bottom().saturating_sub(1) {
            let middle = line::HORIZONTAL.repeat(usize::from(rule.width.saturating_sub(2)));
            let joined = format!("{}{middle}{}", line::VERTICAL_RIGHT, line::VERTICAL_LEFT);
            buf.set_string(rule.x, rule.y, joined, self.theme.border);
        }

        cursor
    }

    /// One Folder in `row`: its icon and label, then its Unread count over
    /// its total, or the total alone, muted, when nothing is Unread.
    fn folder(&self, buf: &mut Buffer, row: Rect, folder: &Shown, style: Style) {
        let mut count = StackStr::<48>::new();
        let mut count_style = style;
        if let Some(Counts { unread, total }) = folder.counts() {
            if unread > 0 {
                write!(count, "{unread} / {total}").ok();
            } else {
                write!(count, "{total}").ok();
                count_style = style.patch(self.theme.muted);
            }
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
        buf.set_stringn(
            count_x,
            row.y,
            count.as_str(),
            count_width.into(),
            count_style,
        );
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

/// Tall enough for every Folder, as `popup_area` allows.
fn popup(area: Rect, picker: &FolderPicker) -> Rect {
    let rows = u16::try_from(picker.rows().count().max(1)).unwrap_or(u16::MAX);
    popup_area(area, WIDTH, rows.saturating_add(CHROME))
}

const fn inner(popup: Rect) -> Rect {
    Rect {
        x: popup.x.saturating_add(1 + PADDING),
        y: popup.y.saturating_add(1),
        width: popup.width.saturating_sub(2 + 2 * PADDING + BAR),
        height: popup.height.saturating_sub(2),
    }
}
