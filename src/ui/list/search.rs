use std::{
    collections::{HashMap, HashSet},
    num::NonZeroUsize,
    ops::Range,
    panic,
    sync::Arc,
    thread,
};

use chrono::Local;

use nucleo_matcher::{
    Config, Matcher, Utf32Str, Utf32String, chars,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::{
    cache::{self, Cache, Scope},
    domain::listing::Listing,
    domain::message::Message,
    domain::per_account::PerAccount,
    domain::query::Query,
    domain::shown::Shown,
    domain::thread::{self as threads, Key, Thread},
    ui::list::completion::Completion,
    ui::list::folder_list,
    ui::list::matched::Matched,
};

/// Candidates below this many are scored on one thread.
const MIN_CHUNK: usize = 1024;

/// How many candidates a Search reads from the Cache at most, newest first.
pub const LIMIT: usize = 500;

/// Between two sender names on a card, and in its haystack.
pub const SENDER_GAP: &str = ", ";

/// A Search over every cached Folder: the Query, the Threads matching it,
/// and the cards of those in a window around the selection. See
/// docs/adr/0023-search-finds-candidates-in-fts5-and-ranks-them-with-nucleo.md.
pub struct Search {
    query: String,
    /// `query` cut into its terms at the last run.
    parsed: Query,
    /// The Query of the last run, and the rows of its candidates: a
    /// Message whose flags change while it is open stays in.
    ran: String,
    kept: Vec<i64>,
    completion: Completion,
    /// More candidates matched than `LIMIT`.
    more: bool,
    pattern: Pattern,
    matcher: Matcher,
    /// The candidates' Threads, newest first, each by its key and the row
    /// of its newest candidate.
    threads: Vec<(Key, i64)>,
    /// What each candidate is matched by, with its Thread's index.
    haystacks: Vec<(usize, Utf32String)>,
    /// Threads holding a match, best match first; `None` for an empty Query.
    hits: Option<Vec<usize>>,
    /// The rows whose Threads `listing` holds.
    window: Range<usize>,
    listing: Listing,
    /// The window's Threads, kept for its buffer.
    wanted: Vec<(Key, i64)>,
    /// The Folders searched, and the window's buffers.
    scope: Scope,
    /// The Thread selected at the last `/`.
    pub before: Option<Key>,
    /// Reused for each card's highlight, so drawing does not allocate.
    card: String,
    graphemes: Vec<char>,
    units: Vec<u32>,
}

impl Search {
    /// A Search with an empty Query over every cached Folder, `shown`'s
    /// copies winning (ADR 0010).
    pub fn new(
        caches: &PerAccount<Cache>,
        shown: &Shown,
        accounts: &PerAccount<Arc<str>>,
    ) -> Result<Self, cache::Error> {
        let mut search = Self {
            query: String::new(),
            parsed: Query { terms: Vec::new() },
            ran: String::new(),
            kept: Vec::new(),
            completion: Completion::new(caches, accounts.clone()),
            more: false,
            pattern: Pattern::default(),
            matcher: Matcher::new(Config::DEFAULT),
            threads: Vec::new(),
            haystacks: Vec::new(),
            hits: None,
            window: 0..0,
            listing: Listing::empty(),
            wanted: Vec::new(),
            scope: Scope::empty(),
            before: None,
            card: String::new(),
            graphemes: Vec::new(),
            units: Vec::new(),
        };

        search.run(caches, shown)?;
        search.suggest(caches)?;
        Ok(search)
    }

    pub const fn listing(&self) -> &Listing {
        &self.listing
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// Whether the Query matched more than the candidates shown.
    pub const fn capped(&self) -> bool {
        self.more && !self.query.is_empty()
    }

    pub const fn parsed(&self) -> &Query {
        &self.parsed
    }

    pub const fn completion(&self) -> &Completion {
        &self.completion
    }

    /// Types `c` into the Query; true, as the Query always changes.
    pub fn push(&mut self, c: char) -> bool {
        self.query.push(c);
        true
    }

    /// Erases the Query's last character; whether there was one.
    pub fn pop(&mut self) -> bool {
        self.query.pop().is_some()
    }

    /// Writes the menu's next suggestion into the Query, or with `forward`
    /// false its previous; whether it had one.
    pub fn complete(&mut self, forward: bool) -> bool {
        self.completion.cycle(&mut self.query, forward)
    }

    /// Fills the menu anew for the Query's last word.
    pub fn suggest(&mut self, caches: &PerAccount<Cache>) -> Result<(), cache::Error> {
        self.completion.suggest(&self.query, caches)
    }

    /// Closes the menu; whether it was open.
    pub fn close_menu(&mut self) -> bool {
        self.completion.close()
    }

    /// Asks the Cache for the Query's candidates and ranks them; the next
    /// `fill` reads the window anew.
    pub fn run(&mut self, caches: &PerAccount<Cache>, shown: &Shown) -> Result<(), cache::Error> {
        let again = self.ran == self.query;
        if !again {
            self.ran.clone_from(&self.query);
            self.parsed = Query::parse(&self.query, Local::now());
        }

        self.scope.reload(caches, shown, &self.parsed)?;
        let kept = again.then_some(&*self.kept);
        let found = cache::candidates(caches, &self.scope, (&self.parsed, kept), LIMIT)?;
        self.kept.clear();
        self.kept
            .extend(found.found.iter().map(|candidate| candidate.row));
        self.more = found.more;
        self.threads.clear();
        self.haystacks.clear();
        let mut index = HashMap::new();
        for candidate in found.found {
            let thread = *index.entry(candidate.key).or_insert_with(|| {
                self.threads.push((candidate.key, candidate.row));
                self.threads.len() - 1
            });

            self.haystacks
                .push((thread, Utf32String::from(candidate.text)));
        }

        self.window = 0..0;
        self.rematch();
        Ok(())
    }

    pub fn rows(&self) -> usize {
        self.hits.as_ref().map_or(self.threads.len(), Vec::len)
    }

    pub fn key(&self, row: usize) -> Option<Key> {
        self.threads.get(self.thread(row)?).map(|&(key, _)| key)
    }

    /// The row that shows the Thread `key`.
    pub fn find(&self, key: Key) -> Option<usize> {
        let index = self.threads.iter().position(|&(k, _)| k == key)?;
        let ranked = |hits: &Vec<usize>| hits.iter().position(|&i| i == index);
        self.hits.as_ref().map_or(Some(index), ranked)
    }

    /// The Thread at `row`, when the window holds it.
    pub fn thread_at(&self, row: usize) -> Option<&Thread> {
        self.listing.threads().get(self.window_index(row)?)
    }

    /// Where among the window's Threads the one at `row` is.
    fn window_index(&self, row: usize) -> Option<usize> {
        if !self.window.contains(&row) {
            return None;
        }

        self.listing.find(self.key(row)?)
    }

    /// Reads the window anew around `selected` unless it holds the rows
    /// within `screen` of it already.
    pub fn fill(
        &mut self,
        caches: &PerAccount<Cache>,
        selected: usize,
        screen: usize,
    ) -> Result<(), cache::Error> {
        let Some(window) = folder_list::around(&self.window, selected, screen, self.rows()) else {
            return Ok(());
        };

        self.wanted.clear();
        for row in window.clone() {
            if let Some(&thread) = self.thread(row).and_then(|i| self.threads.get(i)) {
                self.wanted.push(thread);
            }
        }

        self.window = window;
        let (wanted, scope) = (&self.wanted, &mut self.scope);
        self.listing
            .refill(|here, elsewhere| cache::threads(caches, scope, wanted, (here, elsewhere)))
    }

    /// The Threads among `keys` it shows, read whole whether the window
    /// holds them or not.
    pub fn listing_of(
        &mut self,
        caches: &PerAccount<Cache>,
        keys: &HashSet<Key>,
    ) -> Result<Listing, cache::Error> {
        let wanted: Vec<_> = self
            .threads
            .iter()
            .filter(|(key, _)| keys.contains(key))
            .copied()
            .collect();
        let mut listing = Listing::empty();
        let scope = &mut self.scope;
        listing
            .refill(|here, elsewhere| cache::threads(caches, scope, &wanted, (here, elsewhere)))?;
        Ok(listing)
    }

    /// The Messages and Thread of the card at `row`, with its matched units:
    /// none for an empty Query, or when the match is in a Message the card
    /// does not show. Does not allocate once the buffers have grown.
    pub fn card(&mut self, row: usize) -> Option<(&[Message], &Thread, Matched<'_>)> {
        // Not `thread_at`, whose borrow would cover all of `self`.
        let at = self.window_index(row)?;
        let thread = self.listing.threads().get(at)?;
        let messages = self.listing.messages();
        self.units.clear();
        card(&mut self.card, messages, thread);
        let ascii = self.card.is_ascii();
        if self.hits.is_some() {
            let card = if ascii {
                Utf32Str::Ascii(self.card.as_bytes())
            } else {
                self.graphemes.clear();
                self.graphemes.extend(chars::graphemes(&self.card));
                Utf32Str::Unicode(&self.graphemes)
            };

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

        Some((messages, thread, Matched::new(&self.units, ascii)))
    }

    /// The index among `threads` of the Thread shown at `row`.
    fn thread(&self, row: usize) -> Option<usize> {
        let all = || (row < self.threads.len()).then_some(row);
        self.hits
            .as_ref()
            .map_or_else(all, |hits| hits.get(row).copied())
    }

    fn rematch(&mut self) {
        let ranked_words = self.parsed.ranked();
        // `Pattern::new` takes no fzf syntax; `parse` and `reparse` would.
        self.pattern = Pattern::new(
            &ranked_words,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        self.hits = None;
        if self.pattern.atoms.is_empty() {
            return;
        }

        let mut best = vec![None; self.threads.len()];
        for (score, thread) in score(&self.pattern, &self.haystacks) {
            best[thread] = best[thread].max(Some(score));
        }

        // FTS5 found every Thread here. One nucleo cannot score, even ignoring
        // case, matched only in a body: it comes last. One it scores only
        // ignoring case failed smart case, and is dropped.
        let mut case_failed = vec![false; best.len()];
        if ranked_words.chars().any(char::is_uppercase) {
            let ignoring = Pattern::new(
                &ranked_words,
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
            );
            for (thread, haystack) in &self.haystacks {
                if best[*thread].is_none()
                    && ignoring
                        .score(haystack.slice(..), &mut self.matcher)
                        .is_some()
                {
                    case_failed[*thread] = true;
                }
            }
        }

        let mut ranked: Vec<_> = (0..best.len())
            .filter(|&thread| best[thread].is_some() || !case_failed[thread])
            .map(|thread| (best[thread], thread))
            .collect();
        // Threads are newest first, so a tie goes to the newer Thread.
        ranked.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.hits = Some(ranked.into_iter().map(|(_, i)| i).collect());
    }
}

/// The haystacks `pattern` matches, with their scores and Threads. Spread
/// over a thread per core: on one, 10,000 take up to 17 ms, past a frame.
fn score(pattern: &Pattern, haystacks: &[(usize, Utf32String)]) -> Vec<(u32, usize)> {
    let workers = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let chunk = haystacks.len().div_ceil(workers).max(MIN_CHUNK);
    let part = |part: &[(usize, Utf32String)]| {
        let mut matcher = Matcher::new(Config::DEFAULT);
        part.iter()
            .filter_map(|(thread, haystack)| {
                Some((pattern.score(haystack.slice(..), &mut matcher)?, *thread))
            })
            .collect::<Vec<_>>()
    };

    if haystacks.len() <= chunk {
        return part(haystacks);
    }

    thread::scope(|scope| {
        #[expect(
            clippy::needless_collect,
            reason = "every worker starts before the first join"
        )]
        let workers: Vec<_> = haystacks
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

/// Writes into `into` a card's fields joined by a newline, the separator
/// `Matched` counts units across; its sender names are joined by `SENDER_GAP`.
fn card(into: &mut String, messages: &[Message], thread: &Thread) {
    into.clear();
    for (n, &sender) in thread.senders.iter().enumerate() {
        if n > 0 {
            into.push_str(SENDER_GAP);
        }

        into.push_str(threads::sender(&messages[sender]));
    }

    let (oldest, newest) = (&messages[thread.oldest()], &messages[thread.newest()]);
    for field in [&oldest.subject, &newest.snippet] {
        into.push('\n');
        into.push_str(field);
    }
}
