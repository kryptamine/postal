use mail_parser::{Message, PartType};

use crate::{
    domain::html_text,
    render::html_preview::{self, Links},
};

/// Columns an HTML body is laid out in: the line RFC 5322 recommends.
pub const WIDTH: usize = 78;

/// The `text/plain` part of `message` with `\n` line ends, else its HTML one
/// as text laid out in `width` columns, its links as `links` says.
pub fn plain(message: &Message<'_>, width: usize, links: Links) -> String {
    let text = match message.text_part(0).map(|p| &p.body) {
        Some(PartType::Text(text)) => Some(text.replace("\r\n", "\n")),
        _ => None,
    };
    let at = text.as_deref().and_then(html_text::markup_start);
    let text = match (text, at) {
        (Some(text), None) => return text,
        (text, _) => text.unwrap_or_default(),
    };

    // HTML the sender put into the plain part gives way to the HTML part,
    // which holds the same and more; else it is laid out after its words.
    if let Some(PartType::Html(html)) = message.html_part(0).map(|p| &p.body) {
        return laid_out(html, width, links);
    }

    let (words, html) = text.split_at(at.unwrap_or(0));
    format!("{words}{}", laid_out(html, width, links))
}

fn laid_out(html: &str, width: usize, links: Links) -> String {
    html_preview::render(html, width, links).map_or_else(
        |_| String::new(),
        |lines| {
            let each: Vec<&str> = lines.iter().map(|line| line.text.trim_end()).collect();
            each.join("\n")
        },
    )
}
