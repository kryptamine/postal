use std::ops::RangeInclusive;

use crate::domain::thread::Key;

/// The rows of the Message List picked for the next action to take at
/// once: from where `J` or `K` was first pressed to the selection. Its
/// start is held by key, as an Update moves the rows.
#[derive(Default)]
pub struct Picks {
    /// The Thread the range starts at, and the row it was last found at.
    anchor: Option<(Key, usize)>,
}

impl Picks {
    pub const fn is_empty(&self) -> bool {
        self.anchor.is_none()
    }

    pub const fn clear(&mut self) {
        self.anchor = None;
    }

    /// Starts the range at `key`, shown at `row`, unless one is under way.
    pub const fn start(&mut self, key: Key, row: usize) {
        if self.anchor.is_none() {
            self.anchor = Some((key, row));
        }
    }

    /// The rows picked with the selection on row `selected`.
    pub fn range(&self, selected: usize) -> Option<RangeInclusive<usize>> {
        let (_, row) = self.anchor?;
        Some(row.min(selected)..=row.max(selected))
    }

    /// Finds the start again after the rows changed, by `find`; the Picks
    /// are dropped when its Thread is gone.
    pub fn rebase(&mut self, find: impl Fn(Key) -> Option<usize>) {
        self.anchor = self
            .anchor
            .and_then(|(key, _)| find(key).map(|row| (key, row)));
    }
}
