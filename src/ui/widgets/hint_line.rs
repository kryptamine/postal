use std::fmt::Write;

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    widgets::Widget,
};
use tokio::time::Instant;
use unicode_width::UnicodeWidthStr;

use crate::{
    domain::query::Query,
    mail::outage::Outage,
    mail::status::Status,
    mail::update::Unindexed,
    render::icons::Icons,
    render::stack_str::StackStr,
    ui::frame::theme::Theme,
    ui::input::keymap::{self, HELP_HINT, Hint},
    ui::list::search::LIMIT,
    ui::state::mode::Mode,
    ui::state::toast::Toast,
};

const GAP: u16 = 2;

/// The bottom row: key hints on the left, a Toast or else the Status and
/// Outbox count on the right. Hints that do not fit drop from the right.
/// A Search's Query leads the hints, and is all the left shows while typed.
pub struct HintLine<'a> {
    pub theme: &'a Theme,
    pub toast: Option<&'a Toast>,
    pub status: Status,
    /// The address of the Account `status` is of, with several.
    pub account: Option<&'a str>,
    /// The Messages in the Outbox not yet accepted.
    pub outbox: usize,
    pub mode: Mode,
    pub query: Option<&'a str>,
    /// The Query's terms, whose operators it highlights.
    pub parsed: Option<&'a Query>,
    /// The Query matched more than a Search reads.
    pub capped: bool,
    /// How far the search index is from every Message.
    pub unindexed: Unindexed,
    /// What the Status's countdown counts from.
    pub now: Instant,
}

impl Widget for HintLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let row = row(area);
        if row.is_empty() {
            return;
        }

        let status = self.status_text();
        let (right, style) = match self.toast {
            Some(toast) if toast.error => (&*toast.text, self.theme.error),
            Some(toast) => (&*toast.text, Style::new()),
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
        let end = match self.query {
            Some(query) if self.mode.typing() => self.query(buf, row.x, row, query),
            _ => self.hints(buf, row, room),
        };

        if !right.is_empty() {
            let width = u16::try_from(right.width()).unwrap_or(u16::MAX);
            let at = row
                .right()
                .saturating_sub(width)
                .max(end.saturating_add(GAP))
                .min(row.right());
            let room = usize::from(row.right() - at);
            buf.set_stringn(at, row.y, right, room, style);
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

    /// Where the terminal's cursor goes: after the Query, while it is typed.
    pub fn cursor(&self, area: Rect) -> Option<Position> {
        let query = self.query.filter(|_| self.mode.typing())?;
        let row = row(area);
        let width = u16::try_from(query.width() + 1).unwrap_or(u16::MAX);
        let x = row
            .x
            .saturating_add(width)
            .min(row.right().saturating_sub(1));
        (!row.is_empty()).then_some(Position::new(x, row.y))
    }

    /// The Query, if any, then the hints that fit `room` and `?: help`;
    /// returns where they end.
    fn hints(&self, buf: &mut Buffer, row: Rect, room: usize) -> u16 {
        let mut x = row.x;
        if let Some(query) = self.query {
            x = self.query(buf, x, row, query).saturating_add(GAP);
        }

        let used = usize::from(x - row.x);
        let mut left = room.saturating_sub(used + width(&HELP_HINT));
        let signed_out = self.status == Status::SignedOut;
        for hint in keymap::bindings(self.mode, signed_out).filter_map(|b| b.hint.as_ref()) {
            let needed = width(hint) + usize::from(GAP);
            if needed > left {
                break;
            }

            left -= needed;
            x = self.hint(buf, x, row, hint).saturating_add(GAP);
        }

        self.hint(buf, x.min(row.right()), row, &HELP_HINT)
    }

    /// Draws `/` and `query` from `x`, then whether it matched more than is
    /// shown and how many bodies are not searched yet, cut at the row's
    /// end; returns where they end.
    fn query(&self, buf: &mut Buffer, x: u16, row: Rect, query: &str) -> u16 {
        let room = |x: u16| usize::from(row.right().saturating_sub(x));
        let (x, _) = buf.set_stringn(x, row.y, "/", room(x), self.theme.key);
        let start = x;
        let (x, _) = buf.set_stringn(x, row.y, query, room(x), Style::new());
        for term in self.parsed.iter().flat_map(|parsed| &parsed.terms) {
            let at = |byte: usize| {
                let width = query.get(..byte).map_or(0, UnicodeWidthStr::width);
                start.saturating_add(u16::try_from(width).unwrap_or(u16::MAX))
            };
            let (from, to) = (at(term.head.start).min(x), at(term.head.end).min(x));
            buf.set_style(Rect::new(from, row.y, to - from, 1), self.theme.key);
        }

        let mut notice = StackStr::<96>::new();
        // The texts fit, so the writes cannot fail.
        if self.capped {
            write!(notice, "newest {LIMIT} matches only: type more").ok();
        }

        let Unindexed { bodies, fetching } = self.unindexed;
        if fetching {
            separate(&mut notice);
            notice.write_str("downloading mail").ok();
        }

        if bodies > 0 {
            separate(&mut notice);
            write!(notice, "{bodies} bodies to index").ok();
        }

        if notice.as_str().is_empty() {
            return x;
        }

        let x = x.saturating_add(GAP).min(row.right());
        buf.set_stringn(x, row.y, notice.as_str(), room(x), self.theme.muted)
            .0
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

/// Parts what `notice` holds from what comes next.
fn separate(notice: &mut StackStr<96>) {
    if !notice.as_str().is_empty() {
        notice.write_str(" · ").ok();
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
        Status::SignedOut => Some("\u{f033e}"),
    }
}
