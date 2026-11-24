use std::{fmt::Write, iter};

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    widgets::{Block, Clear, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    render::stack_str::StackStr,
    ui::frame::theme::Theme,
    ui::input::keymap::{self, Binding, Key},
    ui::popup::help::Help,
    ui::state::mode::Mode,
    ui::widgets::{popup_area::popup_area, scrollbar::scrollbar},
};

const TITLE: &str = " Help ";
const GAP: u16 = 2;
/// The border and one column or row of padding inside it, each side.
const FRAME: u16 = 4;
/// The column the scrollbar takes, inside the right padding.
const BAR: u16 = 1;
/// The filter line and the footer, each with a blank row on the body's side.
const CHROME: u16 = 4;
const PLACEHOLDER: &str = " to filter by key or description";
const KEYS: &str = "/ filter · j/k scroll · esc close";
const FILTERING: &str = "type to filter · ↑/↓ scroll · enter keep · esc clear";

/// The keymap of `mode`, grouped as `keymap::bindings` lists it, centred over
/// `area`: a filter line, a heading per group then key, aliases and
/// description per row, scrolled, and the popup's own keys at the bottom.
pub struct HelpPopup<'a> {
    pub theme: &'a Theme,
    pub mode: Mode,
    pub signed_out: bool,
}

impl StatefulWidget for HelpPopup<'_> {
    type State = Help;

    fn render(self, area: Rect, buf: &mut Buffer, help: &mut Help) {
        let bindings = keymap::bindings(self.mode, self.signed_out);
        let keys = column(bindings.clone(), |b| spelled(b.keys));
        let aliases = column(bindings.clone(), |b| spelled(b.aliases));
        let descriptions = column(bindings.clone(), |b| b.description.width());
        let footer = u16::try_from(FILTERING.width()).unwrap_or(u16::MAX);
        let width = (keys + GAP + aliases + GAP + descriptions).max(footer) + FRAME + BAR;
        // The whole keymap's height, so that filtering never resizes it.
        let all = u16::try_from(lines(bindings.clone(), "").count()).unwrap_or(u16::MAX);
        let popup = popup_area(area, width, all.saturating_add(CHROME + FRAME));
        Clear.render(popup, buf);
        Block::bordered()
            .border_style(self.theme.border)
            .render(popup, buf);
        let room = usize::from(popup.width.saturating_sub(2));
        buf.set_stringn(popup.x + 1, popup.y, TITLE, room, self.theme.title);

        let inner = Rect {
            x: popup.x.saturating_add(2),
            y: popup.y.saturating_add(2),
            width: popup.width.saturating_sub(FRAME + BAR),
            height: popup.height.saturating_sub(FRAME),
        };

        if inner.height < CHROME + 1 {
            return;
        }

        self.filter_line(buf, inner, help);
        let footer = if help.filtering { FILTERING } else { KEYS };
        let bottom = inner.bottom() - 1;
        cell(
            buf,
            (inner.x, bottom),
            inner.right(),
            footer,
            self.theme.muted,
        );
        let body = Rect {
            y: inner.y + 2,
            height: inner.height - CHROME,
            ..inner
        };

        let total = lines(bindings.clone(), &help.query).count();
        help.height = usize::from(body.height);
        help.bottom = total.saturating_sub(help.height);
        help.scroll = help.scroll.min(help.bottom);
        if total == 0 {
            let none = "no key matches";
            cell(buf, (body.x, body.y), body.right(), none, self.theme.muted);
            return;
        }

        let shown = lines(bindings, &help.query).skip(help.scroll);
        for (line, y) in shown.zip(body.y..body.bottom()) {
            self.line(buf, (body, y), line, (keys, aliases));
        }

        let track = Rect::new(body.right(), body.y, BAR, body.height);
        scrollbar(buf, track, (help.scroll, total), self.theme.muted);
    }
}

impl HelpPopup<'_> {
    /// `/` and the filter, or what `/` does while there is none; the
    /// terminal's cursor after the filter while it is typed.
    fn filter_line(&self, buf: &mut Buffer, inner: Rect, help: &mut Help) {
        let (x, y, right) = (inner.x, inner.y, inner.right());
        cell(buf, (x, y), right, "/", self.theme.key);
        let x = x.saturating_add(1);
        if help.filtering || !help.query.is_empty() {
            cell(buf, (x, y), right, &help.query, Style::new());
        } else {
            cell(buf, (x, y), right, PLACEHOLDER, self.theme.muted);
        }

        let typed = u16::try_from(help.query.width()).unwrap_or(u16::MAX);
        let at = x.saturating_add(typed).min(right.saturating_sub(1));
        help.cursor = help.filtering.then_some(Position::new(at, y));
    }

    fn line(
        &self,
        buf: &mut Buffer,
        (body, y): (Rect, u16),
        line: Line,
        (keys, aliases): (u16, u16),
    ) {
        let right = body.right();
        let at = |offset| (body.x.saturating_add(offset), y);
        match line {
            Line::Gap => {}
            Line::Group(group) => cell(buf, at(0), right, group, self.theme.title),
            Line::Row(binding) => {
                key_cell(buf, at(0), right, binding.keys, self.theme.key);
                key_cell(buf, at(keys + GAP), right, binding.aliases, self.theme.key);
                let description = at(keys + GAP + aliases + GAP);
                cell(
                    buf,
                    description,
                    right,
                    binding.description,
                    self.theme.muted,
                );
            }
        }
    }
}

/// One row of the body.
#[derive(Clone, Copy)]
enum Line {
    Group(&'static str),
    Row(&'static Binding),
    /// The blank row between groups.
    Gap,
}

/// The body's rows for the bindings `query` keeps: a heading over each
/// group, a blank row between them.
fn lines<'q>(
    bindings: impl Iterator<Item = &'static Binding> + 'q,
    query: &'q str,
) -> impl Iterator<Item = Line> + 'q {
    let kept = bindings.filter(move |b| matches(b, query));
    let headed = kept.scan("", |group, binding| {
        let starts = (binding.group != *group).then_some(group.is_empty());
        *group = binding.group;
        Some((starts, binding))
    });

    headed.flat_map(|(starts, binding)| {
        let gap = (starts == Some(false)).then_some(Line::Gap);
        let head = starts.map(|_| Line::Group(binding.group));
        gap.into_iter()
            .chain(head)
            .chain(iter::once(Line::Row(binding)))
    })
}

/// Whether `query` is in the binding's keys, aliases or description, in
/// any case.
fn matches(binding: &Binding, query: &str) -> bool {
    let spelled = |keys: &[Key]| {
        let mut text = StackStr::<64>::new();
        // A key's name too long for the buffer is left out of the match.
        Key::spell(keys, |part| {
            text.write_str(part).ok();
        });
        contains(text.as_str(), query)
    };

    query.is_empty()
        || contains(binding.description, query)
        || spelled(binding.keys)
        || spelled(binding.aliases)
}

fn contains(haystack: &str, needle: &str) -> bool {
    needle.is_empty()
        || haystack.char_indices().any(|(at, _)| {
            let mut rest = haystack
                .get(at..)
                .unwrap_or_default()
                .chars()
                .flat_map(char::to_lowercase);
            needle
                .chars()
                .flat_map(char::to_lowercase)
                .all(|c| rest.next() == Some(c))
        })
}

/// The widest of one column over `bindings`.
fn column<'a>(
    bindings: impl Iterator<Item = &'a Binding>,
    width: impl Fn(&Binding) -> usize,
) -> u16 {
    bindings
        .map(|b| u16::try_from(width(b)).unwrap_or(u16::MAX))
        .max()
        .unwrap_or(0)
}

fn spelled(keys: &[Key]) -> usize {
    let mut width = 0;
    Key::spell(keys, |part| width += part.width());
    width
}

fn key_cell(buf: &mut Buffer, (mut x, y): (u16, u16), right: u16, keys: &[Key], style: Style) {
    Key::spell(keys, |part| {
        cell(buf, (x, y), right, part, style);
        x = x.saturating_add(u16::try_from(part.width()).unwrap_or(u16::MAX));
    });
}

fn cell(buf: &mut Buffer, (x, y): (u16, u16), right: u16, text: &str, style: Style) {
    if x < right {
        buf.set_stringn(x, y, text, usize::from(right - x), style);
    }
}
