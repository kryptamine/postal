use mail_parser::{Message, PartType};

use crate::html_preview::{self, Links};

/// The `text/plain` part of `message`, else its HTML one as the Text View
/// shows it, laid out in `width` columns.
pub fn plain(message: &Message<'_>, width: usize) -> String {
    if let Some(PartType::Text(text)) = message.text_part(0).map(|p| &p.body) {
        return text.to_string();
    }

    let Some(PartType::Html(html)) = message.html_part(0).map(|p| &p.body) else {
        return String::new();
    };

    html_preview::render(html, width, Links::Inline).map_or_else(
        |_| String::new(),
        |lines| {
            let each: Vec<&str> = lines.iter().map(|line| line.text.trim_end()).collect();
            each.join("\n")
        },
    )
}
