use charset::Charset;

use crate::transfer_encoding::TransferEncoding;

/// The body part a Snippet is cut from and a Preview shows: `text/plain`,
/// else `text/html`.
pub struct TextPart {
    /// The IMAP part number, as in `BODY[1.2]`.
    pub section: String,
    pub encoding: TransferEncoding,
    /// The `charset` parameter as sent; empty when the part has none.
    pub charset: String,
    pub html: bool,
}

impl TextPart {
    /// The text of `raw`, this part's content as sent. An unknown charset is
    /// read as UTF-8, and bytes it cannot hold become U+FFFD.
    pub fn decode(&self, raw: &[u8]) -> String {
        let bytes = self.encoding.decode(raw);
        Charset::for_label(self.charset.as_bytes()).map_or_else(
            || String::from_utf8_lossy(&bytes).into_owned(),
            |charset| charset.decode_without_bom_handling(&bytes).0.into_owned(),
        )
    }
}
