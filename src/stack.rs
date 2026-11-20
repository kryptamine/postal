use std::ops::Range;

use crate::{
    body::Body, inbox_view::InboxView, message::Message, previews::Previews, reading::Reading,
};

/// Rows between two Messages of the stack.
const GAP: usize = 1;

/// One Message of the Thread in Reading, and the rows it takes in the stack.
pub struct Placed<'a> {
    /// Its place in the stack, from the top.
    pub index: usize,
    pub message: &'a Message,
    /// `None` while folded to one line.
    pub body: Option<Body<'a>>,
    pub top: usize,
    pub height: usize,
    /// The header rows above an unfolded body.
    pub header: usize,
}

impl Placed<'_> {
    pub const fn bottom(&self) -> usize {
        self.top + self.height
    }

    /// Whether any of its rows falls in `rows`.
    pub const fn shown(&self, rows: &Range<usize>) -> bool {
        self.top < rows.end && rows.start < self.bottom()
    }

    /// The lines of its rendered body that `rows` of the stack show.
    pub fn body_lines(&self, rows: &Range<usize>) -> Range<usize> {
        let start = self.top + self.header;
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
    let mut top = 0;
    let header = view.header_rows();
    view.stack().enumerate().map(move |(index, message)| {
        let body = reading
            .unfolded(message.body_key)
            .then(|| previews.body(message, view.html, width));
        let height = match &body {
            None => 1,
            Some(Body::Rendered(preview)) => header + preview.body.len(),
            // One row for the placeholder.
            Some(Body::Loading | Body::Unavailable(_)) => header + 1,
        };

        let placed = Placed {
            index,
            message,
            body,
            top,
            height,
            header,
        };

        top += height + GAP;
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

        // The second from the top, else the only one.
        if placed.index <= 1 {
            newest = Some((key, rows));
        }
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
