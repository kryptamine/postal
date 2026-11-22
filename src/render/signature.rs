use std::ops::Range;

use crate::{render::marks::Marks, render::styled_line::StyledLine, render::wrap::wrap};

/// The line a signature starts under (RFC 3676): the space is part of it.
pub const SEPARATOR: &str = "-- ";

/// Plain `shown` wrapped to `width`, with the signature muted: from the
/// last separator line of `own`, the words before a Quote, to their end.
pub fn rows(shown: &str, own: &str, width: usize) -> Vec<StyledLine> {
    let Some(signature) = lines(own) else {
        let rows = wrap(shown, width).into_iter();
        return rows.map(StyledLine::plain).collect();
    };

    let mut rows = Vec::new();
    for (at, line) in shown.lines().enumerate() {
        let wrapped = wrap(line, width);
        // A blank line wraps to nothing.
        if wrapped.is_empty() {
            rows.push(StyledLine::plain(String::new()));
        }

        for row in wrapped {
            let mut row = StyledLine::plain(row);
            if signature.contains(&at) {
                row.restyle(Marks::MUTED);
            }

            rows.push(row);
        }
    }

    while rows.last().is_some_and(|row| row.text.is_empty()) {
        rows.pop();
    }

    rows
}

/// The lines of `own` its signature takes, the separator first.
fn lines(own: &str) -> Option<Range<usize>> {
    let separators = own
        .lines()
        .enumerate()
        .filter(|(_, line)| *line == SEPARATOR);
    let (start, _) = separators.last()?;
    Some(start..own.lines().count())
}
