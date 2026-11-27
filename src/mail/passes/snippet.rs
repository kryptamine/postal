use crate::{domain::html_text, domain::text_part::TextPart};

/// The octets of a text part fetched to cut a Snippet from.
pub const FETCHED: u32 = 2048;

const LENGTH: usize = 200;

/// The first words of `raw`, the start of `part` as the server sent it:
/// decoded, markup stripped, whitespace collapsed to single spaces.
pub fn from_part(raw: &[u8], part: &TextPart) -> String {
    let text = text(raw, part);

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
        Some((end, _)) => snippet.split_at(end).0.trim_end().to_owned(),
        None => snippet,
    }
}

/// The text of `raw`, the start of `part` as the server sent it: decoded
/// and markup stripped.
pub fn text(raw: &[u8], part: &TextPart) -> String {
    let mut text = part.decode(raw);
    // A multi-byte character cut off by the partial fetch decodes as U+FFFD.
    text.truncate(text.trim_end_matches('\u{FFFD}').len());
    // Some senders put HTML into the plain part, often after a first line
    // of words, which stays as written.
    let at = if part.html {
        Some(0)
    } else {
        html_text::markup_start(&text)
    };
    let (words, html) = text.split_at(at.unwrap_or(text.len()));
    format!("{words}{}", html_text::strip(html))
}
