use std::error::Error;

use mail_parser::MessageParser;
use serde::Serialize;

use crate::{
    account::config::Config,
    account::xdg,
    cache::{Reader, ThreadMessage},
    cli::args::ThreadShow,
    cli::command_output,
    compose::notes::Notes,
    domain::answering::Answering,
    render::body_text::{self, WIDTH},
    render::header_date,
    render::html_preview::Links,
};

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
/// answers, oldest first, with its body as plain text; none for a Draft that
/// answers nothing.
pub fn run(show: &ThreadShow) -> Result<(), Box<dyn Error>> {
    let notes = Notes::read(&show.draft)?.unwrap_or_default();
    let thread = match notes.answering {
        Some(answering) => answered(notes.account, &answering)?,
        None => Vec::new(),
    };

    let text = if show.json {
        json(&thread)?
    } else {
        text(&thread)
    };

    Ok(command_output::print(&text)?)
}

/// The Thread `answering` names, in `account` or the first one.
fn answered(
    account: Option<String>,
    answering: &Answering,
) -> Result<Vec<ThreadMessage>, Box<dyn Error>> {
    let account = match account {
        Some(account) => account,
        None => first_account()?,
    };

    let reader = Reader::open(&xdg::cache_dir()?)?;
    let thread = reader
        .thread(&account.to_lowercase(), answering)?
        .ok_or("the Draft answers no Message the Cache knows")?;
    if thread.is_empty() {
        return Err("the Message the Draft answers is not in the Cache".into());
    }

    Ok(thread)
}

/// The Account a Draft whose notes name none goes out from.
fn first_account() -> Result<String, Box<dyn Error>> {
    let configs = Config::load(&xdg::config_dir()?.join("config.toml"))?;
    let first = configs.into_iter().next().ok_or("no Account is set up")?;
    Ok(first.email)
}

/// The body as plain text, without trailing blank lines.
fn body(kept: &ThreadMessage) -> Option<String> {
    let raw = kept.raw.as_deref()?;
    let text = MessageParser::default()
        .parse(raw)
        .map(|message| body_text::plain(&message, WIDTH, Links::Inline))
        .unwrap_or_default();
    Some(text.trim_end().to_owned())
}

fn json(thread: &[ThreadMessage]) -> serde_json::Result<String> {
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
fn text(thread: &[ThreadMessage]) -> String {
    let each: Vec<String> = thread
        .iter()
        .map(|kept| {
            format!(
                "From: {}\nRecipients: {}\nDate: {}\n\n{}\n",
                kept.from,
                kept.to,
                header_date::date(&kept.received).as_str(),
                body(kept).as_deref().unwrap_or(MISSING)
            )
        })
        .collect();
    each.join("\n")
}
