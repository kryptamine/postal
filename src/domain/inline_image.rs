/// A part a Message's HTML shows by `cid:`, decoded.
pub struct InlineImage {
    /// The `Content-ID`, without its angle brackets.
    pub id: String,
    /// Its media type, as `image/png`.
    pub kind: String,
    pub bytes: Vec<u8>,
}
