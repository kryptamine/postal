use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Clear, Widget},
};
use unicode_width::UnicodeWidthStr;

use super::step::Step;
use crate::{account::provider::Provider, render::wrap, ui::frame::theme::Theme};

const TITLE: &str = " Postal ";
const WIDTH: u16 = 72;
/// Columns between the border and the text, each side.
const PADDING: u16 = 2;
const CURSOR: &str = "▏";
const DOT: &str = "•";
/// Apple gives a third-party mail client no OAuth, only this.
const PASSWORD_HELP: &str = "iCloud takes an app-specific password, not your Apple Account one: \
     make one at account.apple.com, under Sign-In and Security.";

/// The Onboarding screen: one bordered box centred over the terminal.
pub struct Page<'a> {
    pub theme: &'a Theme,
    pub step: &'a Step<'a>,
}

enum Row<'a> {
    Blank,
    /// Broken at spaces to the width.
    Text(&'a str, Style),
    /// Broken anywhere, so it reads as one string.
    Url(&'a str),
    Choice(&'a str, bool),
    /// A text field, the cursor at its end; a long one shows its tail.
    Field(&'a str),
    /// A field of this many characters, each drawn as a dot.
    Masked(usize),
    Hints(&'a [(&'a str, &'a str)]),
}

impl Row<'_> {
    fn height(&self, width: u16) -> u16 {
        let lines = match self {
            Self::Text(text, _) => wrap::wrap(text, width.into()).len(),
            Self::Url(url) => url.width().div_ceil(usize::from(width.max(1))),
            Self::Blank | Self::Choice(..) | Self::Field(_) | Self::Masked(_) | Self::Hints(_) => 1,
        };

        u16::try_from(lines).unwrap_or(u16::MAX)
    }
}

impl Widget for Page<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let rows = rows(self.theme, self.step);
        let width = WIDTH.min(area.width);
        let text_width = width.saturating_sub(2 + 2 * PADDING);
        let content: u16 = rows.iter().map(|row| row.height(text_width)).sum();
        // A blank row above and below the text, inside the border.
        let height = content.saturating_add(4).min(area.height);
        let popup = Rect {
            x: area.x + (area.width - width) / 2,
            y: area.y + (area.height - height) / 2,
            width,
            height,
        };

        Clear.render(popup, buf);
        Block::bordered()
            .border_style(self.theme.border)
            .render(popup, buf);
        let room = usize::from(popup.width.saturating_sub(2));
        buf.set_stringn(popup.x + 1, popup.y, TITLE, room, self.theme.title);

        let inner = Rect {
            x: popup.x.saturating_add(1 + PADDING),
            y: popup.y.saturating_add(2),
            width: text_width,
            height: popup.height.saturating_sub(4),
        };

        let mut y = inner.y;
        for row in &rows {
            if y >= inner.bottom() {
                break;
            }

            y = draw(self.theme, row, Rect { y, ..inner }, buf);
        }
    }
}

/// Draws `row` at the top of `area`; returns the row under it.
fn draw(theme: &Theme, row: &Row<'_>, area: Rect, buf: &mut Buffer) -> u16 {
    let width = usize::from(area.width);
    let mut y = area.y;
    match *row {
        Row::Blank => y += 1,
        Row::Text(text, style) => {
            for line in wrap::wrap(text, width) {
                if y < area.bottom() {
                    buf.set_stringn(area.x, y, line, width, style);
                    y += 1;
                }
            }
        }
        Row::Url(url) => {
            // Every byte of a consent URL is ASCII, one column wide.
            let mut rest = url;
            while !rest.is_empty() && y < area.bottom() {
                let (line, after) = rest.split_at(rest.len().min(width.max(1)));
                buf.set_stringn(area.x, y, line, width, theme.link);
                (rest, y) = (after, y + 1);
            }
        }
        Row::Choice(label, selected) => {
            let style = if selected {
                theme.selected
            } else {
                Style::new()
            };
            let line = Rect { height: 1, ..area };
            buf.set_style(line, style);
            buf.set_stringn(area.x + 1, y, label, width.saturating_sub(1), style);
            y += 1;
        }
        Row::Field(value) => {
            field(theme, value, Rect { y, ..area }, buf);
            y += 1;
        }
        Row::Masked(length) => {
            field(theme, &DOT.repeat(length), Rect { y, ..area }, buf);
            y += 1;
        }
        Row::Hints(hints) => {
            let mut x = area.x;
            for &(key, action) in hints {
                let room = usize::from(area.right().saturating_sub(x));
                x = buf.set_stringn(x, y, key, room, theme.key).0;
                let room = usize::from(area.right().saturating_sub(x));
                x = buf
                    .set_stringn(x + 1, y, action, room.saturating_sub(1), theme.muted)
                    .0;
                x = x.saturating_add(3).min(area.right());
            }

            y += 1;
        }
    }

    y
}

/// The steps of a sign-in by an app-specific password; empty for the rest.
fn password_rows<'a>(theme: &Theme, step: &'a Step<'a>) -> Vec<Row<'a>> {
    match *step {
        Step::Address { value } => vec![
            Row::Text("Sign in to iCloud", theme.title),
            Row::Blank,
            Row::Text("Your iCloud address:", theme.muted),
            Row::Field(value),
            Row::Blank,
            Row::Hints(&[("Enter", "next"), ("Esc", "back"), ("Ctrl-C", "cancel")]),
        ],
        Step::Password { heading, length } => vec![
            Row::Text(heading, theme.title),
            Row::Blank,
            Row::Text(PASSWORD_HELP, theme.muted),
            Row::Blank,
            Row::Text("App-specific password:", theme.muted),
            Row::Masked(length),
            Row::Blank,
            Row::Hints(&[("Enter", "sign in"), ("Esc", "back"), ("Ctrl-C", "cancel")]),
        ],
        _ => Vec::new(),
    }
}

/// Draws `value` on the top line of `area`, the cursor in the column after.
fn field(theme: &Theme, value: &str, area: Rect, buf: &mut Buffer) {
    let room = usize::from(area.width).saturating_sub(1);
    let skip = value.width().saturating_sub(room);
    let shown: String = value.chars().skip(skip).collect();
    let x = buf
        .set_stringn(area.x, area.y, &shown, room, Style::new())
        .0;
    buf.set_stringn(x, area.y, CURSOR, 1, theme.key);
}

fn rows<'a>(theme: &Theme, step: &'a Step<'a>) -> Vec<Row<'a>> {
    match *step {
        Step::Providers { selected } => {
            let mut rows = vec![Row::Text("Sign in to your mail", theme.title), Row::Blank];
            rows.extend(
                Provider::ALL
                    .iter()
                    .enumerate()
                    .map(|(index, provider)| Row::Choice(provider.label(), index == selected)),
            );
            rows.extend([
                Row::Blank,
                Row::Hints(&[("↑↓", "choose"), ("Enter", "sign in"), ("q", "quit")]),
            ]);
            rows
        }
        Step::Browser {
            heading,
            url,
            copied,
            back,
        } => {
            let hints: &[(&str, &str)] = match (copied, back) {
                (false, true) => &[("c", "copy link"), ("Esc", "back"), ("Ctrl-C", "cancel")],
                (false, false) => &[("c", "copy link"), ("Ctrl-C", "cancel")],
                (true, true) => &[("c", "copied"), ("Esc", "back"), ("Ctrl-C", "cancel")],
                (true, false) => &[("c", "copied"), ("Ctrl-C", "cancel")],
            };

            vec![
                Row::Text(heading, theme.title),
                Row::Blank,
                Row::Text("If it did not open, visit:", theme.muted),
                Row::Url(url),
                Row::Blank,
                Row::Hints(hints),
            ]
        }
        Step::Address { .. } | Step::Password { .. } => password_rows(theme, step),
        Step::Checking => vec![
            Row::Text("Signing in to the mail server…", theme.title),
            Row::Blank,
            Row::Hints(&[("Ctrl-C", "cancel")]),
        ],
        Step::Replace { question } => vec![
            Row::Text(question, Style::new()),
            Row::Blank,
            Row::Hints(&[("y", "replace"), ("n", "keep it")]),
        ],
        Step::Failed { error } => vec![
            Row::Text("Sign-in failed", theme.error.add_modifier(Modifier::BOLD)),
            Row::Blank,
            Row::Text(error, Style::new()),
            Row::Blank,
            Row::Hints(&[("Enter", "try again"), ("q", "quit")]),
        ],
    }
}
