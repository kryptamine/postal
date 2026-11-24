/// How far the search index is from every Message an Account has.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Indexing {
    /// Cached Messages whose body the index holds.
    pub indexed: usize,
    /// Every cached Message, and those the server holds not cached yet.
    pub total: usize,
}

impl Indexing {
    pub const fn under_way(self) -> bool {
        self.indexed < self.total
    }

    /// The share indexed, rounded down, so 100 only once it is all.
    pub const fn percent(self) -> usize {
        match self.total {
            0 => 100,
            total => self.indexed * 100 / total,
        }
    }

    pub const fn plus(self, other: Self) -> Self {
        Self {
            indexed: self.indexed + other.indexed,
            total: self.total + other.total,
        }
    }
}
