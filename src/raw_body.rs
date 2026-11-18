use crate::text_part::TextPart;

/// A Message's body as the server sent it, with the part a Preview shows.
pub struct RawBody {
    /// `BODY[]`: the header and every part, still MIME-encoded.
    pub raw: Vec<u8>,
    /// `None` when the Message has no text part.
    pub part: Option<TextPart>,
}
