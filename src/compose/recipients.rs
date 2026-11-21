/// Whom a Reply goes to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Recipients {
    Sender,
    /// Everyone the original went to but the Account itself.
    All,
}
