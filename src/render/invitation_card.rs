use std::sync::Arc;

use jiff::{ZonedDateTime, tz::TimeZone};

use crate::render::{
    invitation::{Answer, Invitation, Kind, When},
    marks::Marks,
    paint::Paint,
    styled_line::StyledLine,
    wrap::wrap,
};

/// The columns a label takes, `Invitation` and the space after it.
const LABEL: usize = 11;

/// The card over the body of a Message that carries `invitation`, in
/// `width` columns, times in the local zone; a blank line ends it.
pub fn rows(invitation: &Invitation, width: usize) -> Vec<StyledLine> {
    if let Kind::Reply { attendee, answer } = &invitation.kind {
        let mut line = StyledLine::new();
        line.push(attendee, Marks::STRONG, Paint::default(), None);
        let answered = format!(" {}", replied(*answer));
        line.push(&answered, Marks::default(), Paint::default(), None);
        return vec![line, StyledLine::new()];
    }

    let mut rows = Vec::new();
    let heading = match invitation.kind {
        Kind::Cancel => "Cancelled",
        _ => "Invitation",
    };

    let mut card = Card {
        rows: &mut rows,
        width,
    };

    card.field(heading, &invitation.title, Marks::STRONG);
    if let Some(when) = &invitation.when {
        let line = when_line(when, &TimeZone::system());
        card.field("When", &line, Marks::default());
    }

    if let Some(repeats) = &invitation.repeats {
        card.field("Repeats", repeats, Marks::default());
    }

    if let Some(location) = &invitation.location {
        card.field("Where", location, Marks::default());
    }

    if let Some(join) = &invitation.join {
        card.link("Join", join);
    }

    if let Some(organizer) = &invitation.organizer {
        card.field("Organizer", organizer, Marks::default());
    }

    if let Some(answer) = invitation.answer {
        card.field("You", own(answer), Marks::default());
    }

    rows.push(StyledLine::new());
    rows
}

struct Card<'a> {
    rows: &'a mut Vec<StyledLine>,
    width: usize,
}

impl Card<'_> {
    fn value_width(&self) -> usize {
        self.width.saturating_sub(LABEL).max(10)
    }

    /// `value` wrapped beside `label`, each further line under the first.
    fn field(&mut self, label: &str, value: &str, marks: Marks) {
        let lines = wrap(value, self.value_width());
        self.lines(label, lines.iter().map(String::as_str), marks, None);
    }

    /// `url` cut at the width, each piece a run of the one link.
    fn link(&mut self, label: &str, url: &str) {
        let link: Arc<str> = Arc::from(url);
        let pieces = chunks(url, self.value_width());
        self.lines(label, pieces.into_iter(), Marks::LINK, Some(&link));
    }

    fn lines<'v>(
        &mut self,
        label: &str,
        values: impl Iterator<Item = &'v str>,
        marks: Marks,
        link: Option<&Arc<str>>,
    ) {
        for (index, value) in values.enumerate() {
            let mut row = StyledLine::new();
            let shown = if index == 0 { label } else { "" };
            row.push(
                &format!("{shown:LABEL$}"),
                Marks::MUTED,
                Paint::default(),
                None,
            );
            row.push(value, marks, Paint::default(), link);
            self.rows.push(row);
        }
    }
}

/// `text` in pieces of `width` characters.
fn chunks(text: &str, width: usize) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let cut = rest
            .char_indices()
            .nth(width)
            .map_or(rest.len(), |(at, _)| at);
        let (piece, after) = rest.split_at(cut);
        pieces.push(piece);
        rest = after;
    }

    pieces
}

/// The span in `local`, and in the event's own zone after it when that
/// stands at another offset, with its day when that differs too.
fn when_line(when: &When, local: &TimeZone) -> String {
    match when {
        When::AllDay { first, last } if first == last => {
            format!("{}, all day", first.strftime("%a, %-d %b %Y"))
        }
        When::AllDay { first, last } => format!(
            "{} – {}",
            first.strftime("%a, %-d %b %Y"),
            last.strftime("%a, %-d %b %Y")
        ),
        When::Timed { start, end } => {
            let here = |at: &ZonedDateTime| at.with_time_zone(local.clone());
            let shown = span(&here(start), end.as_ref().map(here).as_ref());
            if here(start).offset() == start.offset() {
                return shown;
            }

            let name = start
                .time_zone()
                .iana_name()
                .map_or_else(|| format!("UTC{}", start.offset()), str::to_owned);
            let theirs = if here(start).date() == start.date() {
                own_span(start, end.as_ref())
            } else {
                span(start, end.as_ref())
            };

            format!("{shown} ({theirs} {name})")
        }
    }
}

/// `Thu, 3 Dec 2026 14:00–15:00`, or both days in full when they differ.
fn span(start: &ZonedDateTime, end: Option<&ZonedDateTime>) -> String {
    let day = start.strftime("%a, %-d %b %Y %H:%M").to_string();
    match end {
        Some(end) if end.date() == start.date() => format!("{day}–{}", end.strftime("%H:%M")),
        Some(end) => format!("{day} – {}", end.strftime("%a, %-d %b %Y %H:%M")),
        None => day,
    }
}

/// The times alone, `15:00–16:00`, or with their days when the span
/// crosses midnight.
fn own_span(start: &ZonedDateTime, end: Option<&ZonedDateTime>) -> String {
    match end {
        Some(end) if end.date() == start.date() => {
            format!("{}–{}", start.strftime("%H:%M"), end.strftime("%H:%M"))
        }
        Some(_) => span(start, end),
        None => start.strftime("%H:%M").to_string(),
    }
}

const fn own(answer: Answer) -> &'static str {
    match answer {
        Answer::Accepted => "accepted",
        Answer::Declined => "declined",
        Answer::Tentative => "maybe",
        Answer::Pending => "not answered yet",
    }
}

const fn replied(answer: Answer) -> &'static str {
    match answer {
        Answer::Accepted => "accepted",
        Answer::Declined => "declined",
        Answer::Tentative => "answered maybe",
        Answer::Pending => "has not answered",
    }
}
