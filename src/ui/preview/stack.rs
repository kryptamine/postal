use std::ops::Range;

use crate::{
    domain::message::Message, render::body::Body, ui::list::inbox_view::InboxView,
    ui::preview::message_header, ui::preview::previews::Previews, ui::preview::reading::Reading,
    ui::preview::stack_line::StackLine,
};

/// Rows between two Messages of the stack.
const GAP: usize = 1;

/// One Message of the Thread in Reading, and the rows it takes in the stack.
pub struct Placed<'a> {
    /// Its place in the stack, from the top.
    pub index: usize,
    /// The first of the Messages it stands for.
    pub message: &'a Message,
    pub shape: Shape<'a>,
    pub top: usize,
    pub height: usize,
}

/// A line of the stack with what the Previews have of an unfolded body.
pub enum Shape<'a> {
    Run(usize),
    Folded,
    Unfolded(Body<'a>),
}

impl Placed<'_> {
    pub const fn bottom(&self) -> usize {
        self.top + self.height
    }

    pub fn header(&self) -> usize {
        message_header::stack_rows(self.message)
    }

    /// Whether any of its rows falls in `rows`.
    pub const fn shown(&self, rows: &Range<usize>) -> bool {
        self.top < rows.end && rows.start < self.bottom()
    }

    /// The top rows of `window` its header covers, pinned there while the
    /// rest of it is scrolled into view; the next Message pushes it up.
    pub fn pinned(&self, window: &Range<usize>) -> usize {
        let scrolled = self.top < window.start && window.start < self.bottom();
        if !matches!(self.shape, Shape::Unfolded(_)) || !scrolled {
            return 0;
        }

        (self.bottom() - window.start).min(self.header())
    }

    /// The lines of its rendered body that `rows` of the stack show.
    pub fn body_lines(&self, rows: &Range<usize>) -> Range<usize> {
        let start = self.top + self.header();
        let lines = |row: usize| row.clamp(start, self.bottom().max(start)) - start;
        lines(rows.start)..lines(rows.end)
    }
}

/// The selected Thread's Messages laid out one under the other at `width`,
/// with what the Previews have of each unfolded one.
pub fn placed<'a>(
    view: &'a InboxView,
    previews: &'a Previews,
    reading: &'a Reading,
    width: u16,
) -> impl Iterator<Item = Placed<'a>> {
    let (mut top, mut index) = (0, 0);
    reading.lines(view.stack()).map(move |(message, line)| {
        let shape = match line {
            StackLine::Run(count) => Shape::Run(count),
            StackLine::Folded => Shape::Folded,
            StackLine::Unfolded => Shape::Unfolded(previews.body(view, message, width)),
        };

        let header = message_header::stack_rows(message);
        let height = match &shape {
            Shape::Run(_) | Shape::Folded => 1,
            Shape::Unfolded(Body::Rendered(preview)) => header + preview.body.len(),
            // One row for the placeholder.
            Shape::Unfolded(Body::Loading | Body::Unavailable(_)) => header + 1,
        };

        let placed = Placed {
            index,
            message,
            shape,
            top,
            height,
        };

        top += height + GAP;
        index += line.messages();
        placed
    })
}

/// Brings the scroll position in range of `height` rows, and the cursor's
/// Message into view after a jump or a fold. After a scroll, the cursor
/// moves to the nearest Message still shown. Messages joining the stack
/// leave the top of the window on the Message it was on.
pub fn settle(
    view: &InboxView,
    previews: &Previews,
    reading: &mut Reading,
    (width, height): (u16, usize),
) {
    let mut total = 0;
    let (mut cursor, mut newest, mut held) = (None, None, None);
    for placed in placed(view, previews, reading, width) {
        total = placed.bottom();
        let rows = placed.top..placed.bottom();
        let key = placed.message.body_key;
        if key == reading.cursor {
            cursor = Some(rows.clone());
        }

        if let Some((top, into)) = reading.top
            && top == key
        {
            held = Some(placed.top + into);
        }

        newest = Some((key, rows));
    }

    let cursor = match (cursor, newest) {
        (Some(cursor), _) => cursor,
        // The cursor's Message left the Thread: the newest takes it.
        (None, Some((key, rows))) => {
            reading.cursor = key;
            rows
        }
        (None, None) => return,
    };

    if std::mem::take(&mut reading.hold)
        && let Some(held) = held
    {
        reading.scroll = held;
    }

    if reading.reveal {
        if cursor.start < reading.scroll {
            reading.scroll = cursor.start;
        } else if cursor.end > reading.scroll + height {
            reading.scroll = cursor.start.min(cursor.end - height);
        }
    }

    reading.scroll = reading.scroll.min(total.saturating_sub(height));
    if !reading.reveal {
        follow(view, previews, reading, (width, height), &cursor);
    }

    let scroll = reading.scroll;
    let top = placed(view, previews, reading, width)
        .find(|p| p.bottom() + GAP > scroll)
        .map(|p| (p.message.body_key, scroll - p.top));
    reading.top = top;
    reading.reached();
}

/// Moves the cursor, at `cursor` rows, to the nearest Message the window shows.
fn follow(
    view: &InboxView,
    previews: &Previews,
    reading: &mut Reading,
    (width, height): (u16, usize),
    cursor: &Range<usize>,
) {
    let window = reading.scroll..reading.scroll + height;
    let shown = || {
        placed(view, previews, reading, width)
            .filter(|p| p.shown(&window))
            .map(|p| p.message.body_key)
    };

    let nearest = if cursor.end <= window.start {
        shown().next()
    } else if cursor.start >= window.end {
        shown().last()
    } else {
        None
    };

    if let Some(key) = nearest {
        reading.cursor = key;
    }
}
