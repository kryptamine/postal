/// Slots kept across refills, so filling them again reuses their buffers.
pub struct Pool<T> {
    items: Vec<T>,
    len: usize,
}

impl<T> Pool<T> {
    pub const fn new() -> Self {
        Self {
            items: Vec::new(),
            len: 0,
        }
    }

    /// Empties it and keeps the slots.
    pub const fn clear(&mut self) {
        self.len = 0;
    }

    /// The next slot as an earlier fill left it, or `new` past the slots kept.
    pub fn next(&mut self, new: impl FnOnce() -> T) -> &mut T {
        if self.len == self.items.len() {
            self.items.push(new());
        }

        self.len += 1;
        &mut self.items[self.len - 1]
    }

    /// Gives the last slot `next` handed out back.
    pub const fn pop(&mut self) {
        self.len = self.len.saturating_sub(1);
    }

    pub fn as_slice(&self) -> &[T] {
        &self.items[..self.len]
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.items[..self.len]
    }
}

impl<T> From<Vec<T>> for Pool<T> {
    fn from(items: Vec<T>) -> Self {
        Self {
            len: items.len(),
            items,
        }
    }
}
