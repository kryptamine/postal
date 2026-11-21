use mail_parser::{Message, PartType};

use crate::html_preview::{self, Links};

/// Columns an HTML body is laid out in: the line RFC 5322 recommends.
pub const WIDTH: usize = 78;

/// The `text/plain` part of `message`, else its HTML one as text laid out
/// in `width` columns, its links as `links` says.
pub fn plain(message: &Message<'_>, width: usize, links: Links) -> String {
    if let Some(PartType::Text(text)) = message.text_part(0).map(|p| &p.body) {
        return text.to_string();
    }

    let Some(PartType::Html(html)) = message.html_part(0).map(|p| &p.body) else {
        return String::new();
    };

    html_preview::render(html, width, links).map_or_else(
        |_| String::new(),
        |lines| {
            let each: Vec<&str> = lines.iter().map(|line| line.text.trim_end()).collect();
            each.join("\n")
        },
    )
}
