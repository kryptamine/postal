use std::borrow::Cow;

use crate::{html_text, text_part::TextPart};

/// The octets of a text part fetched to cut a Snippet from.
pub const FETCHED: u32 = 2048;

const LENGTH: usize = 200;

/// The first words of `raw`, the start of `part` as the server sent it:
/// decoded, markup stripped, whitespace collapsed to single spaces.
pub fn from_part(raw: &[u8], part: &TextPart) -> String {
    let text = part.decode(raw);
    // A multi-byte character cut off by the partial fetch decodes as U+FFFD.
    let text = text.trim_end_matches('\u{FFFD}');
    let text = if part.html {
        Cow::Owned(html_text::strip(text))
    } else {
        Cow::Borrowed(text)
    };

    let mut snippet = String::new();
    for word in text.split(|c: char| c.is_whitespace() || c.is_control()) {
        if word.is_empty() {
            continue;
        }

        if !snippet.is_empty() {
            snippet.push(' ');
        }

        snippet.push_str(word);
        if snippet.chars().count() >= LENGTH {
            break;
        }
    }

    match snippet.char_indices().nth(LENGTH) {
        Some((end, _)) => snippet[..end].trim_end().to_owned(),
        None => snippet,
    }
}
