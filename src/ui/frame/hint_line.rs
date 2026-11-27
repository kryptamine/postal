use std::fmt::Write;

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    widgets::StatefulWidget,
};
use tokio::time::Instant;
use unicode_width::UnicodeWidthStr;

use crate::{
    domain::indexing::Indexing,
    domain::query::Query,
    mail::outage::Outage,
    mail::status::Status,
    render::icons::Icons,
    render::stack_str::StackStr,
    ui::frame::theme::Theme,
    ui::input::keymap::{HELP_HINT, Hint},
    ui::list::search::LIMIT,
    ui::popup::layer,
    ui::state::mode::Mode,
    ui::state::toast::{Text, Toast},
    ui::widgets::{input, truncate},
};

const GAP: u16 = 2;

/// The bottom row: key hints on the left, a Toast or else the Status and
/// Outbox count on the right. Hints that do not fit drop from the right.
/// A Search's Query leads the hints, and is all the left shows while typed.
/// Its state is where the terminal's cursor goes: after the Query, while typed.
pub struct HintLine<'a> {
    pub theme: &'a Theme,
    pub toast: Option<&'a Toast>,
    pub status: Status,
    /// The address of the Account `status` is of, with several.
    pub account: Option<&'a str>,
    /// The Messages in the Outbox not yet accepted.
    pub outbox: usize,
    pub hints: Hints,
    pub query: Option<&'a str>,
    /// The Query's terms, whose operators it highlights.
    pub parsed: Option<&'a Query>,
    /// The Query matched more than a Search reads.
    pub capped: bool,
    /// How far the search index is from every Message.
    pub indexing: Indexing,
    /// How many Threads are picked in the Message List.
    pub picks: usize,
    /// What the Status's countdown counts from.
    pub now: Instant,
}

/// Where the hint line's keys come from.
#[derive(Clone, Copy)]
pub enum Hints {
    /// The keymap's for the mode, after the Query and the picks.
    Keys(Mode),
    /// A layer's own, while it takes every key.
    Fixed(&'static [Hint]),
}

impl StatefulWidget for HintLine<'_> {
    type State = Option<Position>;

    fn render(self, area: Rect, buf: &mut Buffer, cursor: &mut Option<Position>) {
        let row = row(area);
        if row.is_empty() {
            return;
        }

        let status = self.status_text();
        // Written only while a held Send counts down.
        let countdown;
        let (right, style) = match self.toast.map(|toast| (toast, &toast.text)) {
            Some((toast, Text::Said(text))) if toast.error => (text.as_ref(), self.theme.error),
            Some((_, Text::Said(text))) => (text.as_ref(), Style::new()),
            Some((toast, Text::Held)) => {
                countdown = toast.countdown(self.now);
                (countdown.as_str(), Style::new())
            }
            None if self.status == Status::SignedOut => (status.as_str(), self.theme.error),
            None if self.status.warns() => (status.as_str(), self.theme.warning),
            None => (status.as_str(), self.theme.muted),
        };

        let reserved = if right.is_empty() {
            0
        } else {
            right.width() + usize::from(GAP)
        };

        let room = usize::from(row.width).saturating_sub(reserved);
        let end = match (self.hints, self.query) {
            // The last hint needs no gap after it.
            (Hints::Fixed(hints), _) => self
                .fitting(buf, row, room + usize::from(GAP), hints.iter())
                .min(row.right()),
            (Hints::Keys(_), Some(query)) if self.typing() => {
                let (end, at) = self.query(buf, row.x, row, query);
                *cursor = at;
                end
            }
            (Hints::Keys(mode), _) => self.hints(buf, row, room, mode),
        };

        if !right.is_empty() {
            let width = u16::try_from(right.width()).unwrap_or(u16::MAX);
            let at = row
                .right()
                .saturating_sub(width)
                .max(end.saturating_add(GAP))
                .min(row.right());
            let right_area = Rect::new(at, row.y, row.right().saturating_sub(at), 1);
            truncate::draw(buf, right_area, right, style);
        }
    }
}

impl HintLine<'_> {
    /// The Status, then the Outbox count while it is not empty.
    fn status_text(&self) -> StackStr<200> {
        let status = self.status.text(self.now);
        let mut text = StackStr::new();
        // Every text fits, so the writes cannot fail; an address too long is left out.
        if let Some(icon) = status_icon(self.theme.icons, self.status) {
            write!(text, "{icon} ").ok();
        }

        if let Some(account) = self.account.filter(|a| a.len() <= 120) {
            write!(text, "{account} · ").ok();
        }

        text.write_str(status.as_str()).ok();
        if self.outbox > 0 {
            let gap = if status.as_str().is_empty() {
                ""
            } else {
                " · "
            };

            write!(text, "{gap}{} outbox", self.outbox).ok();
        }

        text
    }

    const fn typing(&self) -> bool {
        matches!(self.hints, Hints::Keys(mode) if mode.typing())
    }

    /// Draws from the start of `row` each of `hints` that fits `left` columns
    /// with a gap after it; returns where the next would go.
    fn fitting<'h>(
        &self,
        buf: &mut Buffer,
        row: Rect,
        mut left: usize,
        hints: impl Iterator<Item = &'h Hint>,
    ) -> u16 {
        let mut x = row.x;
        for hint in hints {
            let needed = width(hint) + usize::from(GAP);
            if needed > left {
                break;
            }

            left -= needed;
            x = self.hint(buf, x, row, hint).saturating_add(GAP);
        }

        x
    }

    /// The Query, if any, then the hints that fit `room` and `?: help`;
    /// returns where they end.
    fn hints(&self, buf: &mut Buffer, row: Rect, room: usize, mode: Mode) -> u16 {
        let mut x = row.x;
        if let Some(query) = self.query {
            x = self.query(buf, x, row, query).0.saturating_add(GAP);
        }

        if self.picks > 0 {
            let mut picks = StackStr::<32>::new();
            // The text fits, so the write cannot fail.
            write!(picks, "{} picked", self.picks).ok();
            let room = usize::from(row.right().saturating_sub(x));
            let (end, _) = buf.set_stringn(x, row.y, picks.as_str(), room, self.theme.key);
            x = end.saturating_add(GAP).min(row.right());
        }

        let used = usize::from(x - row.x);
        let left = room.saturating_sub(used + width(&HELP_HINT));
        let signed_out = self.status == Status::SignedOut;
        let hints = layer::bindings(mode, signed_out).filter_map(|b| b.hint.as_ref());
        let rest = Rect {
            x,
            width: row.right().saturating_sub(x),
            ..row
        };

        x = self.fitting(buf, rest, left, hints);
        self.hint(buf, x.min(row.right()), row, &HELP_HINT)
    }

    /// Draws `/` and `query` from `x`, then whether it matched more than is
    /// shown and how far the search index is, cut at the row's end; returns
    /// where they end, and where the cursor goes while the Query is typed.
    fn query(&self, buf: &mut Buffer, x: u16, row: Rect, query: &str) -> (u16, Option<Position>) {
        let room = |x: u16| usize::from(row.right().saturating_sub(x));
        let (x, _) = buf.set_stringn(x, row.y, "/", room(x), self.theme.key);
        let start = x;
        let typing = self.typing();
        let shown = if typing {
            input::shown(query, row.right().saturating_sub(x))
        } else {
            query
        };

        let skipped = query.len() - shown.len();
        let (x, _) = buf.set_stringn(x, row.y, shown, room(x), Style::new());
        let cursor = (typing && x < row.right()).then_some(Position::new(x, row.y));
        for term in self.parsed.iter().flat_map(|parsed| &parsed.terms) {
            if term.head.end <= skipped {
                continue;
            }

            let from_byte = term.head.start.saturating_sub(skipped);
            let to_byte = term.head.end.saturating_sub(skipped);
            let at = |byte: usize| {
                let width = shown.get(..byte).map_or(0, UnicodeWidthStr::width);
                start.saturating_add(u16::try_from(width).unwrap_or(u16::MAX))
            };
            let (from, to) = (at(from_byte).min(x), at(to_byte).min(x));
            buf.set_style(Rect::new(from, row.y, to - from, 1), self.theme.key);
        }

        let mut notice = StackStr::<96>::new();
        // The texts fit, so the writes cannot fail.
        if self.capped {
            write!(notice, "newest {LIMIT} matches only: type more  ").ok();
        }

        // A word not found yet may only be waiting for its body.
        if self.indexing.under_way() {
            let Indexing { indexed, total } = self.indexing;
            write!(notice, "search index {}% (", self.indexing.percent()).ok();
            grouped(&mut notice, indexed);
            notice.write_str(" of ").ok();
            grouped(&mut notice, total);
            notice.write_str(")").ok();
        }

        let notice = notice.as_str().trim_end();
        if notice.is_empty() {
            return (x, cursor);
        }

        let x = x.saturating_add(GAP).min(row.right());
        let (end, _) = buf.set_stringn(x, row.y, notice, room(x), self.theme.muted);
        (end, cursor)
    }

    /// Draws `hint` from `x`, cut at the row's end; returns where it ends.
    fn hint(&self, buf: &mut Buffer, x: u16, row: Rect, hint: &Hint) -> u16 {
        let room = |x: u16| usize::from(row.right().saturating_sub(x));
        let (x, _) = buf.set_stringn(x, row.y, hint.keys, room(x), self.theme.key);
        let (x, _) = buf.set_stringn(x, row.y, " ", room(x), self.theme.muted);
        buf.set_stringn(x, row.y, hint.description, room(x), self.theme.muted)
            .0
    }
}

/// Writes `n` with its thousands parted by commas, as `120,312`.
fn grouped(into: &mut StackStr<96>, n: usize) {
    let (high, low) = (n / 1000, n % 1000);
    if high == 0 {
        write!(into, "{low}").ok();
    } else {
        grouped(into, high);
        write!(into, ",{low:03}").ok();
    }
}

/// The row inside the line's one-column margins.
fn row(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.min(1),
        ..area
    }
}

fn width(hint: &Hint) -> usize {
    hint.keys.width() + 1 + hint.description.width()
}

/// Before the Status's text; `None` in the plain look and while live.
const fn status_icon(icons: Icons, status: Status) -> Option<&'static str> {
    if matches!(icons, Icons::Plain) {
        return None;
    }

    match status {
        Status::Live | Status::Fetching => None,
        Status::Syncing { .. } => Some("\u{f04e6}"),
        Status::Connecting => Some("\u{f0318}"),
        Status::Retrying {
            outage: Outage::Offline,
            ..
        } => Some("\u{f0319}"),
        Status::Retrying {
            outage: Outage::Busy,
            ..
        } => Some("\u{f051f}"),
        Status::Retrying {
            outage: Outage::Failed,
            ..
        } => Some("\u{f0028}"),
        Status::SignedOut => Some("\u{f033e}"),
    }
}
