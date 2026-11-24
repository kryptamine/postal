use std::{collections::HashSet, ops::Range};

use crate::{
    cache::{self, Arrivals, Cache},
    domain::listing::Listing,
    domain::per_account::PerAccount,
    domain::shown::Shown,
    domain::thread::{Key, Thread},
    domain::thread_row::ThreadRow,
};

/// Screens of cards read either side of the one shown, so that scrolling
/// mostly reads no Cache.
const MARGIN: usize = 3;

/// The shown Folder's Message List: every Thread's row, and the cards of
/// those in a window around the selection. See
/// docs/adr/0022-the-message-list-holds-thread-keys-and-reads-cards-for-the-window.md.
pub struct FolderList {
    rows: Vec<ThreadRow>,
    /// All Inboxes' edge in seconds: Messages arrived before it stay out
    /// of the cards as of the rows (ADR 0014).
    since: Option<i64>,
    /// The rows whose Threads `listing` holds.
    window: Range<usize>,
    listing: Listing,
    /// The window's keys, kept for its buffer.
    wanted: HashSet<Key>,
}

impl FolderList {
    pub fn new(rows: Vec<ThreadRow>, since: Option<i64>) -> Self {
        Self {
            rows,
            since,
            window: 0..0,
            listing: Listing::empty(),
            wanted: HashSet::new(),
        }
    }

    /// Takes the rows the Cache holds now; the next `fill` reads the
    /// window again into the same buffers.
    pub fn reset(&mut self, rows: Vec<ThreadRow>, since: Option<i64>) {
        self.rows = rows;
        self.since = since;
        self.window = 0..0;
    }

    /// Its rows' buffer, emptied, for the Cache to read the rows into anew.
    pub fn take_rows(&mut self) -> Vec<ThreadRow> {
        let mut rows = std::mem::take(&mut self.rows);
        rows.clear();
        rows
    }

    pub const fn rows(&self) -> usize {
        self.rows.len()
    }

    /// Scans the rows: well under a millisecond at 100,000.
    pub fn find(&self, key: Key) -> Option<usize> {
        self.rows.iter().position(|row| row.key == key)
    }

    pub fn key(&self, row: usize) -> Option<Key> {
        self.rows.get(row).map(|row| row.key)
    }

    /// The Thread at `row`, when the window holds it.
    pub fn thread(&self, row: usize) -> Option<&Thread> {
        if !self.window.contains(&row) {
            return None;
        }

        let at = self.listing.find(self.rows.get(row)?.key)?;
        self.listing.threads().get(at)
    }

    pub const fn listing(&self) -> &Listing {
        &self.listing
    }

    /// Reads the window anew around `selected` unless it holds the rows
    /// within `screen` of it already; `shown` names each Account's Folder.
    pub fn fill(
        &mut self,
        caches: &PerAccount<Cache>,
        shown: &Shown,
        selected: usize,
        screen: usize,
    ) -> Result<(), cache::Error> {
        let Some(window) = around(&self.window, selected, screen, self.rows.len()) else {
            return Ok(());
        };

        self.wanted.clear();
        self.wanted
            .extend(self.rows[window.clone()].iter().map(|row| row.key));
        let arrivals = Arrivals {
            within: self.rows[window.end - 1].newest.0..=self.rows[window.start].newest.0,
            since: self.since.unwrap_or(i64::MIN),
        };

        let wanted = &self.wanted;
        self.window = window;
        self.listing.refill(|here, elsewhere| {
            for (account, cache) in caches.iter() {
                if let Some(folder) = shown.of(account) {
                    let keep = |key| wanted.contains(&key);
                    cache.window(folder, &arrivals, keep, (&mut *here, &mut *elsewhere))?;
                }
            }

            Ok(())
        })
    }

    /// The Threads among `keys` its rows hold, read whole whether the
    /// window holds them or not; `shown` names each Account's Folder.
    pub fn listing_of(
        &self,
        caches: &PerAccount<Cache>,
        shown: &Shown,
        keys: &HashSet<Key>,
    ) -> Result<Listing, cache::Error> {
        let mut newest = self
            .rows
            .iter()
            .filter(|row| keys.contains(&row.key))
            .map(|row| row.newest.0);
        let mut listing = Listing::empty();
        let Some(first) = newest.next() else {
            return Ok(listing);
        };

        let (from, to) = newest.fold((first, first), |(lo, hi), at| (lo.min(at), hi.max(at)));
        let arrivals = Arrivals {
            within: from..=to,
            since: self.since.unwrap_or(i64::MIN),
        };

        let filled: Result<(), cache::Error> = listing.refill(|here, elsewhere| {
            for (account, cache) in caches.iter() {
                if let Some(folder) = shown.of(account) {
                    let keep = |key| keys.contains(&key);
                    cache.window(folder, &arrivals, keep, (&mut *here, &mut *elsewhere))?;
                }
            }

            Ok(())
        });

        filled.map(|()| listing)
    }
}

/// The rows of `all` a window around `selected` reads, for a list `screen`
/// cards tall; `None` while `held` has those within a screen of it.
pub fn around(
    held: &Range<usize>,
    selected: usize,
    screen: usize,
    all: usize,
) -> Option<Range<usize>> {
    let near = selected.saturating_sub(screen)..(selected + screen + 1).min(all);
    let holds = held.start <= near.start && near.end <= held.end;
    if holds && !held.is_empty() || near.is_empty() {
        return None;
    }

    let margin = MARGIN * screen;
    Some(selected.saturating_sub(margin)..(selected + margin + 1).min(all))
}
