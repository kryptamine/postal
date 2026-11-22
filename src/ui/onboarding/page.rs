use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Clear, Widget},
};
use unicode_width::UnicodeWidthStr;

use super::{
    server_form::{SLOTS, ServerForm, Shown},
    step::Step,
};
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
    Entry(Entry<'a>),
}

/// A labelled row of the server form; `blank` stands in for empty text.
struct Entry<'a> {
    label: &'a str,
    shown: Shown<'a>,
    blank: &'a str,
    focused: bool,
}

impl Row<'_> {
    fn height(&self, width: u16) -> u16 {
        let lines = match self {
            Self::Text(text, _) => wrap::wrap(text, width.into()).len(),
            Self::Url(url) => url.width().div_ceil(usize::from(width.max(1))),
            Self::Blank
            | Self::Choice(..)
            | Self::Field(_)
            | Self::Masked(_)
            | Self::Hints(_)
            | Self::Entry(_) => 1,
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
        Row::Entry(ref row) => {
            entry(theme, row, Rect { y, ..area }, buf);
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

/// The steps of a sign-in by password; empty for the rest.
fn password_rows<'a>(theme: &Theme, step: &'a Step<'a>) -> Vec<Row<'a>> {
    let icloud = |provider| provider == Provider::ICloud;
    match *step {
        Step::Address { value, provider } => vec![
            Row::Text(
                if icloud(provider) {
                    "Sign in to iCloud"
                } else {
                    "Sign in to your mail server"
                },
                theme.title,
            ),
            Row::Blank,
            Row::Text(
                if icloud(provider) {
                    "Your iCloud address:"
                } else {
                    "Your address:"
                },
                theme.muted,
            ),
            Row::Field(value),
            Row::Blank,
            Row::Hints(&[("Enter", "next"), ("Esc", "back"), ("Ctrl-C", "cancel")]),
        ],
        Step::Password {
            heading,
            length,
            provider,
        } => {
            let mut rows = vec![Row::Text(heading, theme.title), Row::Blank];
            if icloud(provider) {
                rows.extend([Row::Text(PASSWORD_HELP, theme.muted), Row::Blank]);
            }

            let label = if icloud(provider) {
                "App-specific password:"
            } else {
                "Password:"
            };

            rows.extend([
                Row::Text(label, theme.muted),
                Row::Masked(length),
                Row::Blank,
                Row::Hints(&[("Enter", "sign in"), ("Esc", "back"), ("Ctrl-C", "cancel")]),
            ]);
            rows
        }
        Step::Servers { heading, form } => server_rows(theme, heading, form),
        Step::LookingUp { heading } => vec![
            Row::Text(heading, theme.title),
            Row::Blank,
            Row::Hints(&[("Esc", "skip"), ("Ctrl-C", "cancel")]),
        ],
        _ => Vec::new(),
    }
}

fn server_rows<'a>(theme: &Theme, heading: &'a str, form: &'a ServerForm) -> Vec<Row<'a>> {
    let mut rows = vec![Row::Text(heading, theme.title), Row::Blank];
    if let Some(source) = form.source {
        rows.extend([Row::Text(source.label(), theme.muted), Row::Blank]);
    }

    rows.extend(SLOTS.iter().enumerate().map(|(at, &slot)| {
        Row::Entry(Entry {
            label: slot.label(),
            shown: form.shown(slot),
            blank: slot.blank(),
            focused: at == form.focus,
        })
    }));

    rows.push(Row::Blank);
    if let Some(problem) = form.problem {
        rows.extend([Row::Text(problem, theme.error), Row::Blank]);
    }

    rows.push(Row::Hints(&[
        ("Tab", "field"),
        ("←→", "change"),
        ("Enter", "next"),
        ("Esc", "back"),
    ]));
    rows
}

/// Columns of a form row's label, before its value.
const LABEL: u16 = 16;

/// A form row: its label, then its value, the cursor after the focused one.
fn entry(theme: &Theme, row: &Entry<'_>, area: Rect, buf: &mut Buffer) {
    let &Entry {
        label,
        ref shown,
        blank,
        focused,
    } = row;

    let label_style = if focused { theme.key } else { theme.muted };
    buf.set_stringn(area.x, area.y, label, usize::from(LABEL), label_style);
    let value = Rect {
        x: area.x.saturating_add(LABEL),
        width: area.width.saturating_sub(LABEL),
        height: 1,
        ..area
    };

    match *shown {
        Shown::Choice(security) if focused => {
            let text = format!("‹ {} ›", security.label());
            buf.set_stringn(value.x, value.y, text, value.width.into(), theme.key);
        }
        Shown::Choice(security) => {
            let room = value.width.into();
            buf.set_stringn(value.x + 2, value.y, security.label(), room, Style::new());
        }
        Shown::Text("") | Shown::Dots(0) if !focused => {
            let room = value.width.into();
            buf.set_stringn(value.x, value.y, format!("({blank})"), room, theme.muted);
        }
        Shown::Text(text) if focused => field(theme, text, value, buf),
        Shown::Dots(length) if focused => field(theme, &DOT.repeat(length), value, buf),
        Shown::Text(text) => {
            buf.set_stringn(value.x, value.y, text, value.width.into(), Style::new());
        }
        Shown::Dots(length) => {
            let room = value.width.into();
            buf.set_stringn(value.x, value.y, DOT.repeat(length), room, Style::new());
        }
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
        Step::Address { .. }
        | Step::Password { .. }
        | Step::Servers { .. }
        | Step::LookingUp { .. } => password_rows(theme, step),
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
