use std::fmt;

/// Text formatted on the stack, so a frame can `write!` without allocating.
/// A write that does not fit fails and leaves the text as it was.
pub struct StackStr<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> StackStr<N> {
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).unwrap_or_default()
    }
}

impl<const N: usize> fmt::Write for StackStr<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        self.bytes
            .get_mut(self.len..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}
