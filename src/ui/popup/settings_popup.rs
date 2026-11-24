use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Modifier, Style},
    widgets::{Block, Clear, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    ui::frame::theme::Theme,
    ui::popup::settings::{Row, Settings},
    ui::popup::settings_section::Section,
    ui::popup::settings_value::Value,
    ui::widgets::{popup_area::popup_area, truncate},
};

const TITLE: &str = "Settings";
const WIDTH: u16 = 64;
/// The border and a blank row or column inside it, each side.
const FRAME: u16 = 4;
/// The title, the tabs, the rule, then the tab's heading and description,
/// each pair with a blank row after it.
const HEADER: u16 = 7;
/// A blank row, the question or failure, the keys, the buttons.
const FOOTER: u16 = 4;
/// The fewest rows a tab's list takes, so a short one keeps the box.
const LIST: u16 = 3;
const GAP: u16 = 2;
/// Where a row's text starts: after a column, the marker, and a gap.
const TEXT: u16 = 3;
const MARKER: &str = "▸";
const CURRENT: &str = "✓";
const ESC: &str = "esc";
const CLOSE: &str = "close";
const CANCEL: &str = "cancel";

/// Settings centred over `area`, a tab per Section: the tab's heading and
/// what it sets, its rows, a line for a question or a failure, its keys,
/// then a button for `Enter` and one for `Esc`.
pub struct SettingsPopup<'a> {
    pub theme: &'a Theme,
    pub settings: &'a Settings,
}

impl Widget for SettingsPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = popup(area, self.settings);
        Clear.render(popup, buf);
        Block::bordered()
            .border_style(self.theme.border)
            .render(popup, buf);
        let inner = inner(popup);
        if inner.height < HEADER + FOOTER + 1 || inner.width < 20 {
            return;
        }

        truncate::draw(buf, line(inner, inner.y), TITLE, self.theme.title);
        self.tabs(buf, line(inner, inner.y + 1));
        let rule = "─".repeat(usize::from(inner.width));
        buf.set_string(inner.x, inner.y + 2, rule, self.theme.border);
        let section = self.settings.section();
        let strong = Style::new().add_modifier(Modifier::BOLD);
        truncate::draw(buf, line(inner, inner.y + 4), section.label(), strong);
        let at = line(inner, inner.y + 5);
        truncate::draw(buf, at, section.about(), self.theme.muted);

        let list = Rect {
            y: inner.y + HEADER,
            height: inner.height - HEADER - FOOTER,
            ..inner
        };
        self.list(buf, list);
        self.footer(buf, inner);
    }
}

impl SettingsPopup<'_> {
    /// A label per Section, the one shown selected.
    fn tabs(&self, buf: &mut Buffer, row: Rect) {
        let mut x = row.x;
        for section in Section::ALL {
            let style = if section == self.settings.section() {
                self.theme.chosen
            } else {
                self.theme.muted
            };

            let label = format!(" {} ", section.label());
            let room = usize::from(row.right().saturating_sub(x));
            (x, _) = buf.set_stringn(x, row.y, &label, room, style);
            x = x.saturating_add(1);
        }
    }

    /// The tab's rows, scrolled to keep the selected one in view.
    fn list(&self, buf: &mut Buffer, list: Rect) {
        let settings = self.settings;
        let selected = settings.selected();
        let offset = offset(settings, list);
        let rows = settings.rows().skip(offset);
        for (row, y) in rows.zip(list.y..list.bottom()) {
            let chosen = row == selected;
            let style = if chosen {
                self.theme.chosen
            } else {
                Style::new()
            };

            let area = line(list, y);
            buf.set_style(area, style);
            let marker = if chosen { MARKER } else { " " };
            buf.set_stringn(area.x + 1, y, marker, 1, style);
            let text = Rect {
                x: area.x + TEXT,
                width: area.width.saturating_sub(TEXT + 1),
                ..area
            };

            self.row(buf, text, row, style);
        }
    }

    /// One row in `area`: its label, then what it holds, right-aligned.
    fn row(&self, buf: &mut Buffer, area: Rect, row: Row, style: Style) {
        let settings = self.settings;
        let muted = style.patch(self.theme.muted);
        let (label, detail, detail_style) = match row {
            Row::Account(account) => {
                let (email, provider) = &settings.accounts[account];
                (email.as_str(), provider.service(), style)
            }
            Row::Signature(account) => match settings.signatures.get(account) {
                Some(Some(first)) => ("  Signature", first.as_str(), style),
                _ => ("  Signature", "none", muted),
            },
            Row::Add => ("+ Add account", "", style),
            Row::Name => match (&settings.editing, &settings.name) {
                (Some(typed), _) => (typed.as_str(), "", style),
                (None, Some(name)) => (name.as_str(), "", style),
                (None, None) => ("none", "", muted),
            },
            Row::Choice(value) => return self.choice(buf, area, value, style),
        };

        let width = u16::try_from(detail.width())
            .unwrap_or(u16::MAX)
            .min(area.width);
        let label_area = Rect {
            width: area.width.saturating_sub(width + 1),
            ..area
        };

        truncate::draw(buf, label_area, label, style);
        let detail_area = Rect {
            x: area.right() - width,
            width,
            ..area
        };

        truncate::draw(buf, detail_area, detail, detail_style);
    }

    /// `value`, a tick when it is in effect, then what it does, muted, in
    /// a column of its own.
    fn choice(&self, buf: &mut Buffer, area: Rect, value: Value, style: Style) {
        let (x, _) = buf.set_stringn(area.x, area.y, value.key(), area.width.into(), style);
        if self.settings.in_effect(value) {
            let room = usize::from(area.right().saturating_sub(x + 1));
            buf.set_stringn(x + 1, area.y, CURRENT, room, style);
        }

        let values = Value::of(self.settings.section());
        let widest = values.iter().map(|v| v.key().width()).max().unwrap_or(0);
        let column = u16::try_from(widest).unwrap_or(u16::MAX) + 2 + GAP;
        let detail = Rect {
            x: area.x.saturating_add(column),
            width: area.width.saturating_sub(column),
            ..area
        };

        if !detail.is_empty() {
            truncate::draw(buf, detail, value.detail(), style.patch(self.theme.muted));
        }
    }

    /// The question or the failure, the keys, then the buttons centred.
    fn footer(&self, buf: &mut Buffer, inner: Rect) {
        let settings = self.settings;
        let bottom = inner.bottom();
        let message = line(inner, bottom - 3);
        if let Some(account) = settings.unlinking {
            let email = settings.accounts.get(account).map_or("", |a| &a.0);
            let question = format!("Unlink {email} and delete its Cache? y/n");
            truncate::draw(buf, message, &question, self.theme.warning);
        } else if let Some(failure) = &settings.failure {
            let first = failure.lines().next().unwrap_or_default();
            truncate::draw(buf, message, first, self.theme.error);
        }

        let keys = match (settings.editing.is_some(), settings.section()) {
            (true, _) => "type the Name",
            (false, Section::Accounts) => "↑↓ select  tab section  d unlink",
            (false, _) => "↑↓ select  tab section",
        };
        truncate::draw(buf, line(inner, bottom - 2), keys, self.theme.muted);

        let close = if settings.editing.is_some() {
            CANCEL
        } else {
            CLOSE
        };
        let primary = primary(settings);
        let width = |text: &str| u16::try_from(text.width()).unwrap_or(u16::MAX);
        let total = primary.map_or(0, |p| width(p) + GAP) + width(ESC) + 1 + width(close);
        let mut x = inner.x + inner.width.saturating_sub(total) / 2;
        let y = bottom - 1;
        let mut put = |text: &str, style: Style| {
            let room = usize::from(inner.right().saturating_sub(x));
            (x, _) = buf.set_stringn(x, y, text, room, style);
        };
        if let Some(primary) = primary {
            put(primary, self.theme.chosen);
            put("  ", Style::new());
        }

        // Plain beside the filled primary: a reversed dim one runs together.
        put(ESC, self.theme.key);
        put(" ", Style::new());
        put(close, Style::new());
    }

    /// Where the terminal's cursor goes while the Name is typed: after it.
    pub fn cursor(&self, area: Rect) -> Option<Position> {
        let typed = self.settings.editing.as_ref()?;
        let inner = inner(popup(area, self.settings));
        let typed = u16::try_from(typed.width()).unwrap_or(u16::MAX);
        let x = (inner.x + TEXT).saturating_add(typed);
        let y = inner.y + HEADER;
        (x < inner.right() && y < inner.bottom()).then(|| Position::new(x, y))
    }
}

/// What `Enter` does on the selected row, as its button reads; none on
/// an Account's own row.
fn primary(settings: &Settings) -> Option<&'static str> {
    match settings.selected() {
        Row::Account(_) => None,
        Row::Name if settings.editing.is_some() => Some(" ↵ save "),
        Row::Signature(_) | Row::Name => Some(" ↵ edit "),
        Row::Add => Some(" ↵ add "),
        Row::Choice(_) => Some(" ↵ apply "),
    }
}

/// The first row shown, keeping the selected one in `list`.
fn offset(settings: &Settings, list: Rect) -> usize {
    let at = settings
        .rows()
        .position(|row| row == settings.selected())
        .unwrap_or(0);
    (at + 1).saturating_sub(usize::from(list.height))
}

const fn line(inner: Rect, y: u16) -> Rect {
    Rect {
        y,
        height: 1,
        ..inner
    }
}

/// Tall enough for the longest tab, so switching never resizes it.
fn popup(area: Rect, settings: &Settings) -> Rect {
    let longest = Section::ALL
        .iter()
        .map(|&section| settings.rows_of(section).count())
        .max()
        .unwrap_or(0);
    let rows = u16::try_from(longest).unwrap_or(u16::MAX).max(LIST);
    popup_area(area, WIDTH, rows.saturating_add(FRAME + HEADER + FOOTER))
}

const fn inner(popup: Rect) -> Rect {
    Rect {
        x: popup.x.saturating_add(2),
        y: popup.y.saturating_add(2),
        width: popup.width.saturating_sub(FRAME),
        height: popup.height.saturating_sub(FRAME),
    }
}
