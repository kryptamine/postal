/// A Folder's Messages as the server last counted them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Counts {
    pub unread: usize,
    pub total: usize,
}

impl Counts {
    pub const fn plus(self, other: Self) -> Self {
        Self {
            unread: self.unread + other.unread,
            total: self.total + other.total,
        }
    }
}
