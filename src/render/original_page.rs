use base64::{Engine, engine::general_purpose::STANDARD};
use mail_parser::{MessageParser, MessagePart, PartType};

use crate::domain::inline_image::InlineImage;

/// The byte order mark settles the encoding ahead of any charset the mail
/// declares; `mail_parser` has decoded the part to UTF-8 already.
const BOM: &str = "\u{feff}";

/// Mail is untrusted and the page opens from disk: no script runs and no
/// form posts. Remote images and styles still load, as the sender meant.
const POLICY: &str = "<meta http-equiv=\"Content-Security-Policy\" \
    content=\"script-src 'none'; object-src 'none'; form-action 'none'\">";

/// A Message as its sender made it, for the browser. A cached body holds
/// the text parts only (docs/adr/0024), so its `cid:` images come apart.
pub enum Original {
    Html(String),
    /// A Message with no HTML part.
    Text(String),
}

impl Original {
    pub fn parse(raw: &[u8]) -> Self {
        let parsed = MessageParser::default().parse(raw);
        let part = |at: Option<&MessagePart<'_>>| match at.map(|part| &part.body) {
            Some(PartType::Html(html)) => Some(Self::Html(html.to_string())),
            Some(PartType::Text(text)) => Some(Self::Text(text.to_string())),
            _ => None,
        };

        // A Message without an HTML part answers `html_part` with its text.
        parsed
            .as_ref()
            .and_then(|message| part(message.html_part(0)).or_else(|| part(message.text_part(0))))
            .unwrap_or_else(|| Self::Text(String::new()))
    }

    /// The `Content-ID`s the HTML part names in `cid:` URLs, each once.
    pub fn inline_ids(&self) -> Vec<String> {
        let Self::Html(html) = self else {
            return Vec::new();
        };

        let mut ids: Vec<String> = Vec::new();
        for (_, id) in cid_urls(html) {
            if !ids.iter().any(|seen| seen == id) {
                ids.push(id.to_owned());
            }
        }

        ids
    }

    /// The page: the HTML part as sent with `images` in place of their
    /// `cid:` URLs, else the text part as preformatted text.
    pub fn page(&self, images: &[InlineImage]) -> String {
        let body = match self {
            Self::Html(html) => inlined(html, images),
            Self::Text(text) => format!(
                "<pre style=\"white-space: pre-wrap\">{}</pre>",
                escaped(text)
            ),
        };

        format!("{BOM}{}", with_policy(&body))
    }
}

/// Each `cid:` URL in `html`: where it starts, and the id after `cid:`.
fn cid_urls(html: &str) -> impl Iterator<Item = (usize, &str)> {
    html.match_indices("cid:").filter_map(|(at, scheme)| {
        let rest = html.get(at + scheme.len()..)?;
        let end = rest
            .find(|c: char| matches!(c, '"' | '\'' | '>' | ')' | '<') || c.is_whitespace())
            .unwrap_or(rest.len());
        let id = rest.get(..end)?;
        (!id.is_empty()).then_some((at, id))
    })
}

fn inlined(html: &str, images: &[InlineImage]) -> String {
    let mut page = String::with_capacity(html.len());
    let mut copied = 0;
    for (at, id) in cid_urls(html) {
        let Some(image) = images.iter().find(|image| image.id == id) else {
            continue;
        };

        page.push_str(html.get(copied..at).unwrap_or_default());
        page.push_str("data:");
        page.push_str(&image.kind);
        page.push_str(";base64,");
        page.push_str(&STANDARD.encode(&image.bytes));
        copied = at + "cid:".len() + id.len();
    }

    page.push_str(html.get(copied..).unwrap_or_default());
    page
}

/// The policy goes after a doctype: anything before one puts the page in
/// quirks mode, which would change its layout.
fn with_policy(html: &str) -> String {
    let rest = html.trim_start();
    let doctype = rest
        .get(..9)
        .is_some_and(|head| head.eq_ignore_ascii_case("<!doctype"));
    match rest.split_once('>') {
        Some((doctype_tag, page)) if doctype => format!("{doctype_tag}>{POLICY}{page}"),
        _ => format!("{POLICY}{html}"),
    }
}

fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }

    out
}
