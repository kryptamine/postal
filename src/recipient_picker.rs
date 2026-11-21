use crate::correspondent::{self, Correspondent};

/// The Correspondents one recipient of a Draft could expand to, narrowed
/// by a typed Query, and the row selected.
pub struct RecipientPicker {
    /// The recipient as written.
    pub entry: String,
    matches: Vec<Correspondent>,
    query: String,
    /// Indices into `matches`, best first.
    hits: Vec<usize>,
    /// Index into `hits`.
    selected: usize,
}

impl RecipientPicker {
    pub fn new(entry: String, matches: Vec<Correspondent>) -> Self {
        let hits = (0..matches.len()).collect();
        Self {
            entry,
            matches,
            query: String::new(),
            hits,
            selected: 0,
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
        self.narrow();
    }

    pub fn pop(&mut self) {
        self.query.pop();
        self.narrow();
    }

    pub const fn down(&mut self) {
        if self.selected + 1 < self.hits.len() {
            self.selected += 1;
        }
    }

    pub const fn up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn chosen(&self) -> Option<&Correspondent> {
        self.hits.get(self.selected).map(|&n| &self.matches[n])
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = &Correspondent> {
        self.hits.iter().map(|&n| &self.matches[n])
    }

    pub const fn selected(&self) -> usize {
        self.selected
    }

    fn narrow(&mut self) {
        let query: Vec<String> = correspondent::words(&self.query).collect();
        self.hits = (0..self.matches.len())
            .filter(|&n| self.matches[n].matches(&query))
            .collect();
        self.selected = 0;
    }
}
