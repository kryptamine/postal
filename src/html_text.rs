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

/// The text of an HTML fragment: tags, comments and hidden elements dropped,
/// entities decoded. A tag cut off at the end is dropped with what follows.
pub fn strip(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find(['<', '&']) {
        text.push_str(&rest[..at]);
        rest = &rest[at..];
        if rest.starts_with('&') {
            let (decoded, len) = entity(rest).unwrap_or(('&', 1));
            text.push(decoded);
            rest = &rest[len..];
            continue;
        }

        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }

        let Some(end) = rest.find('>') else {
            return text;
        };

        let tag = &rest[1..end];
        rest = &rest[end + 1..];
        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !closing && HIDDEN.contains(&name.as_str()) {
            rest = after_closing(rest, &name);
        } else if BREAKING.contains(&name.as_str()) {
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

    html[start..]
        .find('>')
        .map_or("", |end| &html[start + end + 1..])
}

/// The character an entity at the start of `html` stands for, and the
/// entity's length; `None` when it is not one Postal knows.
fn entity(html: &str) -> Option<(char, usize)> {
    let end = html.bytes().take(12).position(|b| b == b';')?;
    let name = &html[1..end];
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
