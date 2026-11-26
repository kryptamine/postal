use base64::{Engine, engine::general_purpose::STANDARD};
use mail_parser::{MessageParser, PartType};

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
pub struct Original {
    html: Option<String>,
    text: String,
}

impl Original {
    pub fn parse(raw: &[u8]) -> Self {
        let parsed = MessageParser::default().parse(raw);
        let html = parsed
            .as_ref()
            .and_then(|message| match &message.html_part(0)?.body {
                PartType::Html(html) => Some(html.to_string()),
                _ => None,
            });
        let text = parsed
            .as_ref()
            .and_then(|message| match &message.text_part(0)?.body {
                PartType::Text(text) => Some(text.to_string()),
                _ => None,
            });

        Self {
            html,
            text: text.unwrap_or_default(),
        }
    }

    /// The `Content-ID`s the HTML part names in `cid:` URLs, each once.
    pub fn inline_ids(&self) -> Vec<String> {
        let Some(html) = &self.html else {
            return Vec::new();
        };

        let mut ids: Vec<String> = Vec::new();
        for (at, _) in html.match_indices("cid:") {
            let rest = html.get(at + 4..).unwrap_or_default();
            let end = rest
                .find(|c: char| matches!(c, '"' | '\'' | '>' | ')' | '<') || c.is_whitespace())
                .unwrap_or(rest.len());
            let id = rest.get(..end).unwrap_or_default();
            if !id.is_empty() && !ids.iter().any(|seen| seen == id) {
                ids.push(id.to_owned());
            }
        }

        ids
    }

    /// The page: the HTML part as sent with `images` in place of their
    /// `cid:` URLs, else the text part as preformatted text.
    pub fn page(&self, images: &[InlineImage]) -> String {
        let body = self.html.as_ref().map_or_else(
            || {
                format!(
                    "<pre style=\"white-space: pre-wrap\">{}</pre>",
                    escaped(&self.text)
                )
            },
            |html| inlined(html, images),
        );

        format!("{BOM}{}", with_policy(&body))
    }
}

fn inlined(html: &str, images: &[InlineImage]) -> String {
    let mut images: Vec<_> = images.iter().collect();
    // `cid:logo` would otherwise cut into `cid:logo2`.
    images.sort_by_key(|image| std::cmp::Reverse(image.id.len()));

    let mut html = html.to_owned();
    for image in images {
        let url = format!(
            "data:{};base64,{}",
            image.kind,
            STANDARD.encode(&image.bytes)
        );
        html = html.replace(&format!("cid:{}", image.id), &url);
    }

    html
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
