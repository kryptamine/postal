use std::ops::Range;

use crate::{
    body::Body, inbox_view::InboxView, message::Message, preview_pane::HEADER_ROWS,
    previews::Previews, reading::Reading,
};

/// Rows between two Messages of the stack.
const GAP: usize = 1;

/// One Message of the Thread in Reading, and the rows it takes in the stack.
pub struct Placed<'a> {
    /// Its place in the stack, oldest first.
    pub index: usize,
    pub message: &'a Message,
    /// `None` while folded to one line.
    pub body: Option<Body<'a>>,
    pub top: usize,
    pub height: usize,
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
        let start = self.top + HEADER_ROWS;
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
    view.stack().enumerate().map(move |(index, message)| {
        let body = reading
            .unfolded(message.uid)
            .then(|| previews.body(message, view.html, width));
        let height = match &body {
            None => 1,
            Some(Body::Rendered(preview)) => HEADER_ROWS + preview.body.len(),
            // One row for the placeholder.
            Some(Body::Loading | Body::Unavailable(_)) => HEADER_ROWS + 1,
        };

        let placed = Placed {
            index,
            message,
            body,
            top,
            height,
        };

        top += height + GAP;
        placed
    })
}

/// Brings the scroll position in range of `height` rows, and the cursor's
/// Message into view after a jump or a fold. After a scroll, the cursor
/// moves to the nearest Message still shown.
pub fn settle(
    view: &InboxView,
    previews: &Previews,
    reading: &mut Reading,
    (width, height): (u16, usize),
) {
    let mut total = 0;
    let (mut cursor, mut newest) = (None, None);
    for placed in placed(view, previews, reading, width) {
        total = placed.bottom();
        let rows = placed.top..placed.bottom();
        if placed.message.uid == reading.cursor {
            cursor = Some(rows.clone());
        }

        newest = Some((placed.message.uid, rows));
    }

    let cursor = match (cursor, newest) {
        (Some(cursor), _) => cursor,
        // The cursor's Message left the Thread: the newest takes it.
        (None, Some((uid, rows))) => {
            reading.cursor = uid;
            rows
        }
        (None, None) => return,
    };

    if reading.reveal {
        if cursor.start < reading.scroll {
            reading.scroll = cursor.start;
        } else if cursor.end > reading.scroll + height {
            reading.scroll = cursor.start.min(cursor.end - height);
        }
    }

    reading.scroll = reading.scroll.min(total.saturating_sub(height));
    let window = reading.scroll..reading.scroll + height;
    if reading.reveal {
        return;
    }

    let shown = || {
        placed(view, previews, reading, width)
            .filter(|p| p.shown(&window))
            .map(|p| p.message.uid)
    };

    let nearest = if cursor.end <= window.start {
        shown().next()
    } else if cursor.start >= window.end {
        shown().last()
    } else {
        None
    };

    if let Some(uid) = nearest {
        reading.cursor = uid;
    }
}
