use std::error::Error;

use mail_parser::MessageParser;
use serde::Serialize;

use crate::{
    body_text,
    cache::{Kept, Reader},
    cli::ThreadShow,
    command_output,
    config::Config,
    message_header::DATE,
    notes::Notes,
    xdg,
};

/// Columns an HTML body is laid out in: the line RFC 5322 recommends.
const WIDTH: usize = 78;

/// What the text answer says for a body not in the Cache.
const MISSING: &str = "[body not in the Cache]";

/// One Message of `--json`, whose fields only grow (ADR 0017).
#[derive(Serialize)]
struct Row<'a> {
    from: &'a str,
    /// `To`, then `Cc`, joined by commas.
    recipients: &'a str,
    /// RFC 3339.
    date: String,
    /// `null` for a body not in the Cache.
    body: Option<String>,
}

/// `postal thread show`: every Message of the Thread a Reply's Draft
/// answers, oldest first, with its body as plain text.
pub fn run(show: &ThreadShow) -> Result<(), Box<dyn Error>> {
    let notes = Notes::read(&show.draft)?.unwrap_or_default();
    let answering = notes.answering.ok_or("the Draft answers no Message")?;
    let account = match notes.account {
        Some(account) => account,
        None => first_account()?,
    };

    let reader = Reader::open(&xdg::cache_dir()?)?;
    let thread = reader
        .thread(&account.to_lowercase(), &answering)?
        .ok_or("the Draft answers no Message")?;
    if thread.is_empty() {
        return Err("the Message the Draft answers is not in the Cache".into());
    }

    let text = if show.json {
        json(&thread)?
    } else {
        text(&thread)
    };

    Ok(command_output::print(&text)?)
}

/// The Account a Draft whose notes name none goes out from.
fn first_account() -> Result<String, Box<dyn Error>> {
    let configs = Config::load(&xdg::config_dir()?.join("config.toml"))?;
    let first = configs.into_iter().next().ok_or("no Account is set up")?;
    Ok(first.email)
}

/// The body as plain text, without trailing blank lines.
fn body(kept: &Kept) -> Option<String> {
    let raw = kept.raw.as_deref()?;
    let text = MessageParser::default()
        .parse(raw)
        .map(|message| body_text::plain(&message, WIDTH))
        .unwrap_or_default();
    Some(text.trim_end().to_owned())
}

fn json(thread: &[Kept]) -> serde_json::Result<String> {
    let rows: Vec<Row<'_>> = thread
        .iter()
        .map(|kept| Row {
            from: &kept.from,
            recipients: &kept.to,
            date: kept.received.to_rfc3339(),
            body: body(kept),
        })
        .collect();
    let mut text = serde_json::to_string(&rows)?;
    text.push('\n');
    Ok(text)
}

/// Each Message as a header block, a blank line and its body, a blank line
/// between Messages.
fn text(thread: &[Kept]) -> String {
    let each: Vec<String> = thread
        .iter()
        .map(|kept| {
            format!(
                "From: {}\nRecipients: {}\nDate: {}\n\n{}\n",
                kept.from,
                kept.to,
                kept.received.format(DATE),
                body(kept).as_deref().unwrap_or(MISSING)
            )
        })
        .collect();
    each.join("\n")
}
