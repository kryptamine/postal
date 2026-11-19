use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Clear, Widget},
};
use unicode_width::UnicodeWidthStr;

use super::step::Step;
use crate::{provider::Provider, theme::Theme, wrap};

const TITLE: &str = " Postal ";
const WIDTH: u16 = 72;
/// Columns between the border and the text, each side.
const PADDING: u16 = 2;

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
    Hints(&'a [(&'a str, &'a str)]),
}

impl Row<'_> {
    fn height(&self, width: u16) -> u16 {
        let lines = match self {
            Self::Text(text, _) => wrap::wrap(text, width.into()).len(),
            Self::Url(url) => url.width().div_ceil(usize::from(width.max(1))),
            Self::Blank | Self::Choice(..) | Self::Hints(_) => 1,
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
