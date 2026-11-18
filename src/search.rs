use std::{num::NonZeroUsize, panic, thread};

use nucleo_matcher::{
    Config, Matcher, Utf32String,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::{matched::Matched, message::Message};

/// Candidates below this many are scored on one thread.
const MIN_CHUNK: usize = 1024;

/// A Search over the Message List: the Query and the rows matching it.
pub struct Search {
    query: String,
    pattern: Pattern,
    matcher: Matcher,
    /// Each Message's sender, subject and Snippet, one per row of the full list.
    haystacks: Vec<Utf32String>,
    /// Rows of the full list, best match first; `None` for an empty Query.
    hits: Option<Vec<usize>>,
    /// The Message selected at the last `/`.
    pub before: Option<u32>,
    /// Reused for each card's highlight, so drawing does not allocate.
    units: Vec<u32>,
}

impl Search {
    pub fn new(messages: &[Message]) -> Self {
        Self {
            query: String::new(),
            pattern: Pattern::default(),
            matcher: Matcher::new(Config::DEFAULT),
            haystacks: haystacks(messages),
            hits: None,
            before: None,
            units: Vec::new(),
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// Matches the Query again over `messages`, the new full list.
    pub fn reload(&mut self, messages: &[Message]) {
        self.haystacks = haystacks(messages);
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
        self.hits.as_ref().map_or(self.haystacks.len(), Vec::len)
    }

    /// The full list's row shown at `row`.
    pub fn row(&self, row: usize) -> Option<usize> {
        let all = || (row < self.haystacks.len()).then_some(row);
        self.hits
            .as_ref()
            .map_or_else(all, |hits| hits.get(row).copied())
    }

    /// Where the full list's row `index` is shown.
    pub fn position(&self, index: usize) -> Option<usize> {
        let all = || (index < self.haystacks.len()).then_some(index);
        self.hits
            .as_ref()
            .map_or_else(all, |hits| hits.iter().position(|&i| i == index))
    }

    /// The matched units of the full list's row `index`; none for an empty
    /// Query. Does not allocate once the buffer has grown.
    pub fn matched(&mut self, index: usize) -> Matched<'_> {
        self.units.clear();
        let Some(haystack) = self.haystacks.get(index) else {
            return Matched::new(&[], true);
        };

        if self.hits.is_some() {
            let haystack = haystack.slice(..);
            self.pattern
                .indices(haystack, &mut self.matcher, &mut self.units);
            self.units.sort_unstable();
            self.units.dedup();
        }

        Matched::new(&self.units, matches!(haystack, Utf32String::Ascii(_)))
    }

    /// `narrows` matches only the current hits, a superset of the new ones.
    fn rematch(&mut self, narrows: bool) {
        // `Pattern::new` takes no fzf syntax; `parse` and `reparse` would.
        self.pattern = Pattern::new(
            &self.query,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let previous = self.hits.take();
        if self.pattern.atoms.is_empty() {
            return;
        }

        let candidates = match previous {
            Some(hits) if narrows => hits,
            _ => (0..self.haystacks.len()).collect(),
        };

        let mut scored = score(&self.pattern, &self.haystacks, &candidates);
        // The full list is newest first, so a tie goes to the newer Message.
        scored.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.hits = Some(scored.into_iter().map(|(_, i)| i).collect());
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

/// One haystack per Message: its fields joined by a newline, the separator
/// `Matched` counts units across.
fn haystacks(messages: &[Message]) -> Vec<Utf32String> {
    messages
        .iter()
        .map(|m| Utf32String::from(format!("{}\n{}\n{}", m.sender, m.subject, m.snippet)))
        .collect()
}
