use mail_parser::MessageParser;

use crate::{
    render::body_text::{self, WIDTH},
    render::html_preview::Links,
};

/// What a command says for a body not in the Cache.
pub const MISSING: &str = "[body not in the Cache]";

/// A raw body as plain text, without trailing blank lines.
pub fn plain(raw: &[u8]) -> String {
    let text = MessageParser::default()
        .parse(raw)
        .map(|message| body_text::plain(&message, WIDTH, Links::Inline))
        .unwrap_or_default();
    text.trim_end().to_owned()
}
