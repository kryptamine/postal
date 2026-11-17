use crate::transfer_encoding::TransferEncoding;

/// The body part a Snippet is cut from: `text/plain`, else `text/html`.
pub struct TextPart {
    /// The IMAP part number, as in `BODY[1.2]`.
    pub section: String,
    pub encoding: TransferEncoding,
    /// The `charset` parameter as sent; empty when the part has none.
    pub charset: String,
    pub html: bool,
}
