use std::{num::NonZeroUsize, panic, thread};

use nucleo_matcher::{
    Config, Matcher, Utf32String,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::{
    domain::listing::Listing,
    domain::message::Message,
    domain::thread::{self as threads, Key, Thread},
    ui::matched::Matched,
};

/// Candidates below this many are scored on one thread.
const MIN_CHUNK: usize = 1024;

/// Between two sender names on a card, and in its haystack.
pub const SENDER_GAP: &str = ", ";

/// A Search over every cached Folder: the Query and the Threads matching it.
pub struct Search {
    query: String,
    /// What is searched, from every Folder but Trash and Spam.
    listing: Listing,
    pattern: Pattern,
    matcher: Matcher,
    /// What each Message is matched by, one per Message.
    haystacks: Vec<Utf32String>,
    /// The Thread each Message is in, by index.
    thread_of: Vec<usize>,
    /// Each Thread's card as drawn, for its highlight: senders, subject, Snippet.
    cards: Vec<Utf32String>,
    /// The Messages matching; `None` for an empty Query.
    matching: Option<Vec<usize>>,
    /// Threads holding a match, best match first; `None` for an empty Query.
    hits: Option<Vec<usize>>,
    /// The Thread selected at the last `/`.
    pub before: Option<Key>,
    /// Reused for each card's highlight, so drawing does not allocate.
    units: Vec<u32>,
}

impl Search {
    pub fn new(listing: Listing) -> Self {
        let mut search = Self {
            query: String::new(),
            listing,
            pattern: Pattern::default(),
            matcher: Matcher::new(Config::DEFAULT),
            haystacks: Vec::new(),
            thread_of: Vec::new(),
            cards: Vec::new(),
            matching: None,
            hits: None,
            before: None,
            units: Vec::new(),
        };

        search.index();
        search
    }

    pub const fn listing(&self) -> &Listing {
        &self.listing
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// Matches the Query again over what the Cache holds now.
    pub fn reload(&mut self, listing: Listing) {
        self.listing = listing;
        self.index();
        self.rematch(false);
    }

    pub fn push(&mut self, c: char) {
        // A longer Query matches a subset, unless `c` completes an escape.
        let narrows = !self.query.ends_with('\\');
        self.query.push(c);
        self.rematch(narrows);
    }

    pub fn pop(&mut self) {
        if self.query.pop().is_some() {
            self.rematch(false);
        }
    }

    pub fn rows(&self) -> usize {
        self.hits.as_ref().map_or(self.cards.len(), Vec::len)
    }

    /// The Thread shown at `row`.
    pub fn row(&self, row: usize) -> Option<usize> {
        let all = || (row < self.cards.len()).then_some(row);
        self.hits
            .as_ref()
            .map_or_else(all, |hits| hits.get(row).copied())
    }

    /// Where the Thread `index` is shown.
    pub fn position(&self, index: usize) -> Option<usize> {
        let all = || (index < self.cards.len()).then_some(index);
        self.hits
            .as_ref()
            .map_or_else(all, |hits| hits.iter().position(|&i| i == index))
    }

    /// The Messages and Thread of the card at `row`, with its matched units:
    /// none for an empty Query, or when the match is in a Message the card
    /// does not show. Does not allocate once the buffer has grown.
    pub fn card(&mut self, row: usize) -> Option<(&[Message], &Thread, Matched<'_>)> {
        let index = self.row(row)?;
        let ascii = self.highlight(index)?;
        let thread = self.listing.threads.get(index)?;
        let matched = Matched::new(&self.units, ascii);
        Some((&self.listing.messages, thread, matched))
    }

    /// Fills `units` for Thread `index`'s card; whether that card is ASCII.
    fn highlight(&mut self, index: usize) -> Option<bool> {
        self.units.clear();
        let card = self.cards.get(index)?;
        if self.hits.is_some() {
            let card = card.slice(..);
            // On a miss, the words before the one that missed have left
            // their units behind.
            if self
                .pattern
                .indices(card, &mut self.matcher, &mut self.units)
                .is_none()
            {
                self.units.clear();
            }

            self.units.sort_unstable();
            self.units.dedup();
        }

        Some(matches!(card, Utf32String::Ascii(_)))
    }

    fn index(&mut self) {
        let Listing {
            messages, threads, ..
        } = &self.listing;
        self.haystacks = messages.iter().map(haystack).collect();
        self.thread_of = vec![0; messages.len()];
        for (index, thread) in threads.iter().enumerate() {
            for &message in &thread.messages {
                self.thread_of[message] = index;
            }
        }

        self.cards = threads.iter().map(|t| card(messages, t)).collect();
    }

    /// `narrows` matches only the current matches, a superset of the new ones.
    fn rematch(&mut self, narrows: bool) {
        // `Pattern::new` takes no fzf syntax; `parse` and `reparse` would.
        self.pattern = Pattern::new(
            &self.query,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let previous = self.matching.take();
        self.hits = None;
        if self.pattern.atoms.is_empty() {
            return;
        }

        let candidates = match previous {
            Some(matching) if narrows => matching,
            _ => (0..self.haystacks.len()).collect(),
        };

        let scored = score(&self.pattern, &self.haystacks, &candidates);
        let mut best = vec![None; self.cards.len()];
        for &(score, message) in &scored {
            let thread = &mut best[self.thread_of[message]];
            *thread = (*thread).max(Some(score));
        }

        let mut ranked: Vec<_> = (0..best.len())
            .filter_map(|thread| Some((best[thread]?, thread)))
            .collect();
        // Threads are newest first, so a tie goes to the newer Thread.
        ranked.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.matching = Some(scored.into_iter().map(|(_, i)| i).collect());
        self.hits = Some(ranked.into_iter().map(|(_, i)| i).collect());
    }
}

/// The candidates `pattern` matches, with their scores. Spread over a
/// thread per core: on one, 10,000 Messages take up to 17 ms, past a frame.
fn score(pattern: &Pattern, haystacks: &[Utf32String], candidates: &[usize]) -> Vec<(u32, usize)> {
    let workers = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let chunk = candidates.len().div_ceil(workers).max(MIN_CHUNK);
    let part = |part: &[usize]| {
        let mut matcher = Matcher::new(Config::DEFAULT);
        part.iter()
            .filter_map(|&i| {
                let haystack = haystacks[i].slice(..);
                Some((pattern.score(haystack, &mut matcher)?, i))
            })
            .collect::<Vec<_>>()
    };

    if candidates.len() <= chunk {
        return part(candidates);
    }

    thread::scope(|scope| {
        #[expect(
            clippy::needless_collect,
            reason = "every worker starts before the first join"
        )]
        let workers: Vec<_> = candidates
            .chunks(chunk)
            .map(|chunk| scope.spawn(move || part(chunk)))
            .collect();

        workers
            .into_iter()
            .flat_map(|worker| {
                worker
                    .join()
                    .unwrap_or_else(|panic| panic::resume_unwind(panic))
            })
            .collect()
    })
}

/// What a Query is matched against. Only a sent Message has its recipients
/// in it: mail from others is found by who sent it.
fn haystack(message: &Message) -> Utf32String {
    let mut fields = vec![&*message.sender, &*message.subject, &*message.snippet];
    if message.from_me {
        fields.push(&message.to);
    }

    Utf32String::from(fields.join("\n"))
}

/// A card's fields joined by a newline, the separator `Matched` counts
/// units across; its sender names are joined by `SENDER_GAP`.
fn card(messages: &[Message], thread: &Thread) -> Utf32String {
    let mut card = String::new();
    for (n, &sender) in thread.senders.iter().enumerate() {
        if n > 0 {
            card.push_str(SENDER_GAP);
        }

        card.push_str(threads::sender(&messages[sender]));
    }

    let (oldest, newest) = (&messages[thread.oldest()], &messages[thread.newest()]);
    Utf32String::from(format!("{card}\n{}\n{}", oldest.subject, newest.snippet))
}
