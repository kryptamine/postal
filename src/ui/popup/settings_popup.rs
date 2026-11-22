use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    widgets::{Block, Clear, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    ui::frame::theme::Theme,
    ui::popup::settings::{Row, Settings},
    ui::widgets::truncate,
};

const TITLE: &str = " Settings ";
const WIDTH: u16 = 60;
/// The border and a blank row inside it, top and bottom.
const CHROME: u16 = 4;
/// The two headings, the blank row between the sections and the one above
/// the footer, and the footer.
const FIXED_ROWS: u16 = 5;
/// Columns between the border and a row's text, each side.
const PADDING: u16 = 1;
/// A row's text under its heading.
const INDENT: u16 = 2;

/// Settings centred over `area`: the Accounts, then the General settings,
/// each row's value right-aligned, then a line for a question or a failure.
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
        let room = usize::from(popup.width.saturating_sub(2));
        buf.set_stringn(popup.x + 1, popup.y, TITLE, room, self.theme.title);

        let inner = inner(popup);
        let mut lines = inner.y..inner.bottom().saturating_sub(2);
        let selected = self.settings.selected();
        for row in self.settings.rows() {
            let heading = match row {
                Row::Account(0) => Some("Accounts"),
                Row::Add if self.settings.accounts.is_empty() => Some("Accounts"),
                Row::Icons => Some("General"),
                _ => None,
            };

            if let Some(heading) = heading {
                if row == Row::Icons {
                    lines.next();
                }

                let Some(y) = lines.next() else { break };
                truncate::draw(buf, line(inner, y), heading, self.theme.title);
            }

            let Some(y) = lines.next() else { break };
            let style = if row == selected {
                self.theme.selected
            } else {
                Style::new()
            };

            buf.set_style(Rect::new(popup.x + 1, y, popup.width - 2, 1), style);
            let indented = Rect {
                x: inner.x.saturating_add(INDENT),
                width: inner.width.saturating_sub(INDENT),
                ..line(inner, y)
            };

            self.row(buf, indented, row, style);
        }

        let footer = line(inner, inner.bottom().saturating_sub(1));
        if let Some(account) = self.settings.unlinking {
            let email = self.settings.accounts.get(account).map_or("", |a| &a.0);
            let question = format!("Unlink {email} and delete its Cache? y/n");
            truncate::draw(buf, footer, &question, self.theme.warning);
        } else if let Some(failure) = &self.settings.failure {
            let first = failure.lines().next().unwrap_or_default();
            truncate::draw(buf, footer, first, self.theme.error);
        }
    }
}

impl SettingsPopup<'_> {
    /// One row in `area`: its label, then its value right-aligned.
    fn row(&self, buf: &mut Buffer, area: Rect, row: Row, style: Style) {
        let settings = self.settings;
        let (label, value, muted) = match row {
            Row::Account(account) => {
                let (email, provider) = &settings.accounts[account];
                (email.as_str(), provider.service(), false)
            }
            Row::Signature(account) => match settings.signatures.get(account) {
                Some(Some(first)) => ("  Signature", first.as_str(), false),
                _ => ("  Signature", "none", true),
            },
            Row::Add => ("+ Add account", "", false),
            Row::Icons => ("Icons", settings.icons.key(), false),
            Row::Notify => ("Notify", settings.notify.key(), false),
            Row::Name => match (&settings.editing, &settings.name) {
                (Some(typed), _) => ("Name", typed.as_str(), false),
                (None, Some(name)) => ("Name", name.as_str(), false),
                (None, None) => ("Name", "none", true),
            },
            Row::Secrets => ("Secrets", settings.secrets.key(), false),
        };

        let width = u16::try_from(value.width())
            .unwrap_or(u16::MAX)
            .min(area.width);
        let value_style = if muted {
            style.patch(self.theme.muted)
        } else {
            style
        };

        let label_area = Rect {
            width: area.width.saturating_sub(width + 1),
            ..area
        };

        truncate::draw(buf, label_area, label, style);
        let value_area = Rect {
            x: area.right() - width,
            width,
            ..area
        };

        truncate::draw(buf, value_area, value, value_style);
    }

    /// Where the terminal's cursor goes while the Name is typed: after it.
    pub fn cursor(&self, area: Rect) -> Option<Position> {
        self.settings.editing.as_ref()?;
        let inner = inner(popup(area, self.settings));
        let accounts = u16::try_from(self.settings.accounts.len()).unwrap_or(u16::MAX);
        // Below the Accounts heading, two rows each, Add, a blank row,
        // General, Icons and Notify.
        let y = inner
            .y
            .saturating_add(accounts.saturating_mul(2).saturating_add(6));
        (y < inner.bottom()).then(|| Position::new(inner.right(), y))
    }
}

const fn line(inner: Rect, y: u16) -> Rect {
    Rect {
        y,
        height: 1,
        ..inner
    }
}

/// Tall enough for every row, as `area` allows.
fn popup(area: Rect, settings: &Settings) -> Rect {
    let rows = u16::try_from(settings.rows().count()).unwrap_or(u16::MAX);
    let width = WIDTH.min(area.width);
    let height = rows.saturating_add(FIXED_ROWS + CHROME).min(area.height);
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
        y: popup.y.saturating_add(2),
        width: popup.width.saturating_sub(2 + 2 * PADDING),
        height: popup.height.saturating_sub(CHROME),
    }
}
