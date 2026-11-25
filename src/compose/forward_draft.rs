use std::{fmt::Write, path::Path};

use mail_parser::MessageParser;

use crate::{
    compose::reply_draft::mailboxes, domain::message::Message, render::body_text,
    render::header_date, render::html_preview::Links,
};

/// The line a Forward's Draft sets the original under.
pub const FORWARDED: &str = "---------- Forwarded message ---------";

/// The Draft a Forward of `original` opens on, from its body `raw`: no
/// recipient yet, an `Attach:` line for each file in `attached`, then the
/// `signature` above the original under a forwarded-message header.
pub fn forward(
    raw: &[u8],
    original: &Message,
    signature: Option<&str>,
    attached: &[impl AsRef<Path>],
) -> String {
    let message = MessageParser::default().parse(raw);
    let message = message.as_ref();
    let list = |header| {
        let each: Vec<String> = mailboxes([header])
            .iter()
            .map(ToString::to_string)
            .collect();
        each.join(", ")
    };

    let from = message
        .map(|m| list(m.from()))
        .filter(|from| !from.is_empty())
        .unwrap_or_else(|| original.from.clone());
    let to = message.map_or_else(String::new, |m| list(m.to()));
    let cc = message.map_or_else(String::new, |m| list(m.cc()));
    let subject = message
        .and_then(|m| m.subject())
        .unwrap_or(&original.subject);
    let text = message.map_or_else(String::new, |m| {
        body_text::plain(m, body_text::WIDTH, Links::Inline)
    });

    let date = header_date::date(&original.received);
    let fields = [
        ("From", &*from),
        ("Date", date.as_str()),
        ("Subject", subject),
        ("To", &to),
        ("Cc", &cc),
    ];
    let mut header = String::new();
    for (name, value) in fields.iter().filter(|(_, value)| !value.is_empty()) {
        // Writing to a String cannot fail.
        writeln!(header, "{name}: {value}").ok();
    }

    let mut attach = String::new();
    for path in attached {
        writeln!(attach, "Attach: {}", path.as_ref().display()).ok();
    }

    let signed = signature.map_or_else(String::new, |signature| format!("{signature}\n"));
    format!(
        "To: \nCc: \nSubject: {}\n{attach}\n\n{signed}{FORWARDED}\n{header}\n{}\n",
        self::subject(subject),
        text.trim_end(),
    )
}

/// `Fwd: ` once, however often the Message was forwarded.
fn subject(original: &str) -> String {
    let lower = original.to_ascii_lowercase();
    if lower.starts_with("fwd:") || lower.starts_with("fw:") {
        original.to_owned()
    } else {
        format!("Fwd: {original}")
    }
}
