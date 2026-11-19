use std::fmt::Write;

use chrono::{DateTime, FixedOffset};
use ratatui::{
    buffer::Buffer,
    layout::{Margin, Rect},
    style::{Modifier, Style},
    widgets::{Block, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    border,
    centred::centred,
    date_group::DateGroup,
    date_label::DateLabel,
    inbox_view::InboxView,
    layout::PADDING,
    matched::Matched,
    message::Message,
    search::SENDER_GAP,
    senders::Senders,
    stack_str::StackStr,
    theme::Theme,
    thread::{self, Thread},
    truncate,
};

const MARKER: &str = "●";
/// The marker column and its gap; the subject and Snippet lines indent by the same.
const INDENT: u16 = 2;
const CARD_HEIGHT: u16 = 3;
/// The rule naming a Date Group, above all but the top one.
const GROUP_GAP: u16 = 1;
/// Below this list width the date is dropped. The list beside the Preview
/// is never narrower than this.
const MIN_WIDTH_WITH_DATE: u16 = 29;
/// The date, a `DateLabel` of at most 10 columns, and the gap before it.
const DATE_ROOM: u16 = 11;
pub const MIN_WIDTH: u16 = 20;

/// How many cards a list `height` rows tall shows at most, its border and
/// a cut last card included; Date Group headers can leave room for fewer.
pub fn capacity(height: u16) -> usize {
    usize::from(height.saturating_sub(2).div_ceil(CARD_HEIGHT)).max(1)
}

/// One three-line card per Thread, `● senders (n) … date` / subject / Snippet,
/// under Date Group headers, inside a border; the Preview's title names the
/// Folder.
pub struct MessageList<'a> {
    pub theme: &'a Theme,
}

impl StatefulWidget for MessageList<'_> {
    type State = InboxView;

    fn render(self, area: Rect, buf: &mut Buffer, view: &mut InboxView) {
        if area.width < MIN_WIDTH {
            return centred("terminal too small", area, buf);
        }

        let block = Block::bordered().border_style(self.theme.border);
        let inner = block.inner(area);
        block.render(area, buf);
        let label = view.folder.label();
        let mut text = StackStr::<160>::new();
        let Some(messages) = view.messages.as_deref() else {
            let loading = write!(text, "Loading {label}…").map_or("Loading…", |()| text.as_str());
            return centred(loading, inner, buf);
        };

        if messages.is_empty() {
            let empty = write!(text, "{label} is empty").map_or("empty", |()| text.as_str());
            return centred(empty, inner, buf);
        }

        let rows = view.rows();
        if rows == 0 {
            return centred("no matches", inner, buf);
        }

        let now = view.now();
        let dated = view.search.is_none().then_some(&now);
        if view.selected < view.offset {
            view.offset = view.selected;
        } else {
            view.offset = view.offset.max(first_fitting(view, dated, inner.height));
        }

        // The top group is named on the border itself, as a title.
        if let Some(group) = group(view, dated, view.offset) {
            name_group(self.theme, buf, area, area.y, group);
        }

        let cards = Cards {
            theme: self.theme,
            now: (area.width >= MIN_WIDTH_WITH_DATE).then_some(now),
        };

        draw_rows(view, &cards, dated, area, buf);
    }
}

/// The cards from `view.offset` down inside the bordered `area`, each
/// group's first under a rule naming it.
fn draw_rows(
    view: &mut InboxView,
    cards: &Cards<'_>,
    dated: Option<&DateTime<FixedOffset>>,
    area: Rect,
    buf: &mut Buffer,
) {
    let Some(messages) = view.messages.as_deref() else {
        return;
    };

    let inner = area.inner(Margin::new(1, 1));
    let mut y = inner.y;
    for row in view.offset..view.rows() {
        let starts = (row > view.offset)
            .then(|| starts_group(view, dated, row))
            .flatten();
        let gap = if starts.is_some() { GROUP_GAP } else { 0 };
        // The last card may be cut short; the selected one never is.
        if y + gap >= inner.bottom() {
            break;
        }

        if let Some(group) = starts {
            y += gap;
            border::rule(buf, area, y - 1, cards.theme.border);
            name_group(cards.theme, buf, area, y - 1, group);
        }

        let index = view.search.as_ref().map_or(Some(row), |s| s.row(row));
        let Some(thread) = index.and_then(|i| view.threads.get(i)) else {
            continue;
        };

        let matched = view.search.as_mut().zip(index).map(|(s, i)| s.matched(i));
        let card = Rect::new(inner.x, y, inner.width, CARD_HEIGHT).intersection(inner);
        y += CARD_HEIGHT;
        let shown = Shown {
            messages,
            thread,
            selected: row == view.selected,
        };

        cards.draw(&shown, card, buf);
        if let Some(matched) = &matched {
            cards.highlight(&shown, matched, card, buf);
        }
    }
}

/// `─ Today ─` on border row `y` of `pane`, over a rule or the top border.
fn name_group(theme: &Theme, buf: &mut Buffer, pane: Rect, y: u16, group: DateGroup) {
    let label = group.label();
    let parts = [" ", label.as_str(), " "];
    border::label(buf, pane, (pane.x + 2, y), &parts, theme.title);
}

/// The Date Group of the Thread at `row`; `None` while undated, as a
/// Search is, whose rows are ranked by match rather than by date.
fn group(view: &InboxView, dated: Option<&DateTime<FixedOffset>>, row: usize) -> Option<DateGroup> {
    let received = &view.row(row)?.received;
    dated.map(|now| DateGroup::new(received, now))
}

/// The Date Group that the Thread at `row` starts, below one of another.
fn starts_group(
    view: &InboxView,
    dated: Option<&DateTime<FixedOffset>>,
    row: usize,
) -> Option<DateGroup> {
    let above = group(view, dated, row.checked_sub(1)?);
    group(view, dated, row).filter(|&group| Some(group) != above)
}

/// The lowest first row that keeps the selected card whole on a list
/// `height` rows tall, a gap above each group but the top one.
fn first_fitting(view: &InboxView, dated: Option<&DateTime<FixedOffset>>, height: u16) -> usize {
    let mut used = CARD_HEIGHT;
    let mut first = view.selected;
    while first > 0 {
        let gap = starts_group(view, dated, first).map_or(0, |_| GROUP_GAP);
        if used + CARD_HEIGHT + gap > height {
            break;
        }

        used += CARD_HEIGHT + gap;
        first -= 1;
    }

    first
}

/// One card's Thread, among the Folder's Messages.
struct Shown<'a> {
    messages: &'a [Message],
    thread: &'a Thread,
    selected: bool,
}

impl Shown<'_> {
    fn newest(&self) -> &Message {
        &self.messages[self.thread.newest()]
    }

    fn oldest(&self) -> &Message {
        &self.messages[self.thread.oldest()]
    }
}

/// Draws cards; `now` is `None` when the terminal is too narrow for the date.
struct Cards<'a> {
    theme: &'a Theme,
    now: Option<DateTime<FixedOffset>>,
}

impl Cards<'_> {
    fn draw(&self, shown: &Shown<'_>, card: Rect, buf: &mut Buffer) {
        let (thread, newest) = (shown.thread, shown.newest());
        let base = if shown.selected {
            self.theme.selected
        } else {
            Style::new()
        };

        // The selected card keeps one style throughout: nothing on it is
        // muted or coloured, which would break the selection's colour.
        let tone = |style: Style| {
            if shown.selected {
                base
            } else {
                base.patch(style)
            }
        };

        let strong = if thread.unread {
            base.patch(self.theme.unread)
        } else {
            base
        };

        // The selection fills the whole card; only its text is padded.
        buf.set_style(card, base);
        let text = text_area(card);
        if thread.unread {
            let style = strong.patch(tone(self.theme.marker));
            buf.set_string(text.x, text.y, MARKER, style);
        }

        let [senders, subject, snippet] = self.fields(card);
        let count = count(thread);
        let style = strong.patch(tone(self.theme.sender));
        let end = place(shown, names(senders, &count), |area, name, gap| {
            if let Some((area, gap)) = gap {
                truncate::draw(buf, area, gap, base);
            }

            truncate::draw(buf, area, name.1, style);
        });

        let room = usize::from(senders.right().saturating_sub(end));
        buf.set_stringn(end, text.y, count.as_str(), room, tone(self.theme.muted));
        if let Some(now) = &self.now {
            let label = DateLabel::new(&newest.received, now);
            let date = label.as_str();
            let date_x = text.right().saturating_sub(width(date));
            buf.set_string(date_x, text.y, date, tone(self.theme.date));
        }

        if !subject.is_empty() {
            truncate::draw(buf, subject, &shown.oldest().subject, strong);
        }

        if !snippet.is_empty() {
            truncate::draw(buf, snippet, &newest.snippet, tone(self.theme.muted));
        }
    }

    /// Restyles the characters a Query matched on a drawn card, whose
    /// haystack `Search` joins as the card shows it.
    fn highlight(&self, shown: &Shown<'_>, matched: &Matched<'_>, card: Rect, buf: &mut Buffer) {
        let style = if shown.selected {
            self.theme.selected.add_modifier(Modifier::UNDERLINED)
        } else {
            self.theme.matched
        };

        let [senders, subject_area, snippet_area] = self.fields(card);
        let thread = shown.thread;
        let gap = matched.len(SENDER_GAP);
        let start = |n: usize| -> u32 {
            let name = |i: usize| thread::sender(&shown.messages[thread.senders[i]]);
            (0..n).map(|i| matched.len(name(i)) + gap).sum()
        };

        let count = count(thread);
        place(shown, names(senders, &count), |area, (n, name), _| {
            matched.draw(buf, area, (name, start(n)), style);
        });

        let after = start(thread.senders.len()).saturating_sub(gap) + 1;
        let (subject, snippet) = (&*shown.oldest().subject, &*shown.newest().snippet);
        let starts = matched.starts([subject, snippet]);
        for ((area, text), start) in [(subject_area, subject), (snippet_area, snippet)]
            .into_iter()
            .zip(starts)
        {
            if !area.is_empty() {
                matched.draw(buf, area, (text, after + start), style);
            }
        }
    }

    /// Where a card's senders, subject and Snippet go; a row past the card
    /// is empty. The senders leave room for the date.
    fn fields(&self, card: Rect) -> [Rect; 3] {
        let text = text_area(card);
        let text_width = text.width.saturating_sub(INDENT);
        let date = if self.now.is_some() { DATE_ROOM } else { 0 };
        let x = text.x + INDENT;
        [
            Rect::new(x, text.y, text_width.saturating_sub(date), 1),
            Rect::new(x, text.y + 1, text_width, 1),
            Rect::new(x, text.y + 2, text_width, 1),
        ]
        .map(|field| field.intersection(card))
    }
}

/// ` (n)` after the senders of a Thread of `n` Messages, those in other
/// Folders too; empty for one.
fn count(thread: &Thread) -> StackStr<24> {
    let mut count = StackStr::new();
    if thread.stack.len() > 1 {
        write!(count, " ({})", thread.stack.len()).ok();
    }

    count
}

/// The part of the senders' field left to their names by the count.
fn names(senders: Rect, count: &StackStr<24>) -> Rect {
    Rect {
        width: senders.width.saturating_sub(width(count.as_str())),
        ..senders
    }
}

/// Lays the names `Senders` shows out on `area`, handing `each` every
/// name's area, its place among the senders and the gap drawn before it.
/// Returns the column where they end.
fn place<'a>(
    shown: &Shown<'a>,
    area: Rect,
    mut each: impl FnMut(Rect, (usize, &'a str), Option<(Rect, &'static str)>),
) -> u16 {
    let senders = Senders::fit(shown.messages, shown.thread, usize::from(area.width));
    let gap = senders.gap();
    let mut x = area.x;
    for (n, name) in senders.shown() {
        let before = (x > area.x).then(|| {
            let before = Rect::new(x, area.y, width(gap), 1).intersection(area);
            x = before.right();
            (before, gap)
        });

        let room = area.right().saturating_sub(x);
        if room == 0 {
            break;
        }

        let name_area = Rect::new(x, area.y, width(name).min(room), 1);
        each(name_area, (n, name), before);
        x = name_area.right();
    }

    x
}

fn width(text: &str) -> u16 {
    u16::try_from(text.width()).unwrap_or(u16::MAX)
}

/// A card's text, padded inside the selection's fill.
const fn text_area(card: Rect) -> Rect {
    Rect {
        x: card.x + PADDING,
        width: card.width.saturating_sub(2 * PADDING),
        ..card
    }
}
