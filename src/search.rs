use std::{num::NonZeroUsize, panic, thread};

use nucleo_matcher::{
    Config, Matcher, Utf32String,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::{
    matched::Matched,
    message::Message,
    thread::{self as threads, Key, Thread},
};

/// Candidates below this many are scored on one thread.
const MIN_CHUNK: usize = 1024;

/// Between two sender names on a card, and in its haystack.
pub const SENDER_GAP: &str = ", ";

/// A Search over the Message List: the Query and the Threads matching it.
pub struct Search {
    query: String,
    pattern: Pattern,
    matcher: Matcher,
    /// Each Message's sender, subject and Snippet, one per Message.
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
    pub fn new(messages: &[Message], threads: &[Thread]) -> Self {
        let mut search = Self {
            query: String::new(),
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

        search.index(messages, threads);
        search
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// Matches the Query again over the new full list.
    pub fn reload(&mut self, messages: &[Message], threads: &[Thread]) {
        self.index(messages, threads);
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

    /// The matched units of Thread `index`'s card; none for an empty Query,
    /// or when the match is in a Message the card does not show. Does not
    /// allocate once the buffer has grown.
    pub fn matched(&mut self, index: usize) -> Matched<'_> {
        self.units.clear();
        let Some(card) = self.cards.get(index) else {
            return Matched::new(&[], true);
        };

        if self.hits.is_some() {
            let card = card.slice(..);
            self.pattern
                .indices(card, &mut self.matcher, &mut self.units);
            self.units.sort_unstable();
            self.units.dedup();
        }

        Matched::new(&self.units, matches!(card, Utf32String::Ascii(_)))
    }

    fn index(&mut self, messages: &[Message], threads: &[Thread]) {
        self.haystacks = messages
            .iter()
            .map(|m| Utf32String::from(format!("{}\n{}\n{}", m.sender, m.subject, m.snippet)))
            .collect();
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
