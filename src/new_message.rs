use crate::{message::Message, text_part::TextPart};

/// A Message as the first fetch phase sees it: its row, still without a
/// Snippet, and the part the Snippet is cut from.
pub struct NewMessage {
    pub row: Message,
    /// `None` when the Message has no text part: its Snippet stays empty.
    pub text_part: Option<TextPart>,
}
