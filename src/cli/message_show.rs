use std::{error::Error, fmt::Write};

use serde::Serialize;

use crate::{
    account::xdg,
    cache::Reader,
    cache::ThreadOf,
    cli::accounts,
    cli::answer::Answer,
    cli::args::MessageShow,
    cli::ids,
    cli::plain_body::{self, MISSING},
    render::header_date,
};

/// The Message of `--json`, whose fields only grow (ADR 0017).
#[derive(Serialize)]
struct Row<'a> {
    message: &'a str,
    thread: String,
    account: &'a str,
    /// `null` for one only its Thread in All Mail held.
    folder: Option<&'a str>,
    from: &'a str,
    /// `To`, then `Cc`, joined by commas.
    recipients: &'a str,
    subject: &'a str,
    /// RFC 3339.
    date: String,
    attachments: Vec<Attachment<'a>>,
    /// `null` for a body not in the Cache.
    body: Option<String>,
}

#[derive(Serialize)]
struct Attachment<'a> {
    name: &'a str,
    /// In octets once decoded, as far as the size sent tells.
    size: u64,
}

/// `postal message show`: one Message, its headers and its body as plain
/// text, its attachments by name and size.
pub fn answer(show: &MessageShow) -> Result<Answer, Box<dyn Error>> {
    let emails = accounts::emails()?;
    let (account, key) = ids::parse_message(&show.id, &emails)?;
    let reader = Reader::open(&xdg::cache_dir()?)?;
    let kept = reader
        .message(account, key)?
        .ok_or_else(|| format!("the Cache holds no Message {}", show.id))?;

    let row = Row {
        message: &show.id,
        thread: ids::thread(account, ThreadOf::of(kept.thread, kept.root, key)),
        account,
        folder: kept.folder.as_deref(),
        from: &kept.from,
        recipients: &kept.to,
        subject: &kept.subject,
        date: kept.received.to_rfc3339(),
        attachments: (kept.attachments.iter())
            .map(|a| Attachment {
                name: a.name,
                size: a.size,
            })
            .collect(),
        body: kept.raw.as_deref().map(plain_body::plain),
    };

    let mut text = format!(
        "From: {}\nRecipients: {}\nDate: {}\nSubject: {}\nThread: {}\n",
        row.from,
        row.recipients,
        header_date::date(&kept.received).as_str(),
        row.subject,
        row.thread
    );
    for attachment in &row.attachments {
        // Writing to a `String` cannot fail.
        writeln!(
            text,
            "Attachment: {} ({} bytes)",
            attachment.name, attachment.size
        )
        .ok();
    }

    writeln!(text, "\n{}", row.body.as_deref().unwrap_or(MISSING)).ok();
    Ok(Answer::new(&[&row], text)?)
}
