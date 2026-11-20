/// Elements whose content is never shown as text.
const HIDDEN: [&str; 4] = ["head", "script", "style", "title"];

/// Elements that break a line, so the words on either side stay apart.
const BREAKING: [&str; 20] = [
    "blockquote",
    "br",
    "dd",
    "div",
    "dt",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hr",
    "li",
    "ol",
    "p",
    "table",
    "td",
    "th",
    "tr",
    "ul",
];

/// Whether the element `name` breaks a line, whatever its case.
pub fn breaks(name: &str) -> bool {
    BREAKING.iter().any(|tag| tag.eq_ignore_ascii_case(name))
}

/// The text of an HTML fragment: tags, comments and hidden elements dropped,
/// entities decoded. A tag cut off at the end is dropped with what follows.
pub fn strip(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find(['<', '&']) {
        let (before, from) = rest.split_at(at);
        text.push_str(before);
        rest = from;
        if rest.starts_with('&') {
            let (decoded, len) = entity(rest).unwrap_or(('&', 1));
            text.push(decoded);
            rest = rest.split_at(len).1;
            continue;
        }

        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = comment.split_once("-->").map_or("", |(_, after)| after);
            continue;
        }

        let Some((tag, after)) = rest.split_once('>') else {
            return text;
        };

        let tag = tag.strip_prefix('<').unwrap_or(tag);
        rest = after;
        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !closing && HIDDEN.contains(&name.as_str()) {
            rest = after_closing(rest, &name);
        } else if breaks(&name) {
            text.push(' ');
        }
    }

    text.push_str(rest);
    text
}

/// What follows `</name>`, matched without regard to case.
fn after_closing<'a>(html: &'a str, name: &str) -> &'a str {
    let lower = html.to_ascii_lowercase();
    let closing = format!("</{name}");
    let Some(start) = lower.find(&closing) else {
        return "";
    };

    html.split_at(start)
        .1
        .split_once('>')
        .map_or("", |(_, after)| after)
}

/// The character an entity at the start of `html` stands for, and the
/// entity's length; `None` when it is not one Postal knows.
fn entity(html: &str) -> Option<(char, usize)> {
    let end = html.bytes().take(12).position(|b| b == b';')?;
    let name = html.get(1..end)?;
    let decoded = match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        _ => name
            .strip_prefix("#x")
            .or_else(|| name.strip_prefix("#X"))
            .map_or_else(
                || name.strip_prefix('#').and_then(|n| n.parse().ok()),
                |hex| u32::from_str_radix(hex, 16).ok(),
            )
            .and_then(char::from_u32)?,
    };

    Some((decoded, end + 1))
}
