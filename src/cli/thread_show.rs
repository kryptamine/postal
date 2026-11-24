use std::{error::Error, path::Path};

use serde::Serialize;

use crate::{
    account::xdg,
    cache::{Reader, ThreadMessage},
    cli::accounts,
    cli::answer::{Answer, Answers},
    cli::args::ThreadShow,
    cli::ids,
    cli::plain_body::{self, MISSING},
    compose::notes::Notes,
    domain::answering::Answering,
    domain::body_key::BodyKey,
    domain::thread::Key,
    render::header_date,
};

/// One Message of `--json`, whose fields only grow (ADR 0017).
#[derive(Serialize)]
struct Row<'a> {
    message: String,
    from: &'a str,
    /// `To`, then `Cc`, joined by commas.
    recipients: &'a str,
    subject: &'a str,
    /// RFC 3339.
    date: String,
    /// `null` for a body not in the Cache.
    body: Option<String>,
}

/// `postal thread show`: every Message of the Thread an ID names, a Reply's
/// Draft answers or a Message file is of, oldest first, each body as plain
/// text; none for a Draft that answers nothing.
impl Answers for ThreadShow {
    fn answer(&self) -> Result<Answer, Box<dyn Error>> {
        let (account, thread) = match (&self.id, &self.draft, &self.message) {
            (Some(id), None, None) => named(id)?,
            (None, Some(draft), None) => {
                let notes = Notes::read(draft)?.unwrap_or_default();
                let account = match notes.account {
                    Some(account) => account.to_lowercase(),
                    None => accounts::emails()?
                        .into_iter()
                        .next()
                        .ok_or("no Account is set up")?,
                };

                let thread = match notes.answering {
                    Some(answering) => answered(&account, &answering)?,
                    None => Vec::new(),
                };

                (account, thread)
            }
            (None, None, Some(message)) => opened(message)?,
            _ => return Err("name a Thread by one of its ID, --draft or --message".into()),
        };

        let rows: Vec<Row<'_>> = thread
            .iter()
            .map(|kept| Row {
                message: ids::message(&account, kept.body_key),
                from: &kept.from,
                recipients: &kept.to,
                subject: &kept.subject,
                date: kept.received.to_rfc3339(),
                body: kept.raw.as_deref().map(plain_body::plain),
            })
            .collect();
        Ok(Answer::new(&rows, text(&thread, &rows))?)
    }
}

/// The Account a Thread ID names, and its Thread.
fn named(id: &str) -> Result<(String, Vec<ThreadMessage>), Box<dyn Error>> {
    let emails = accounts::emails()?;
    let key = ids::parse_thread(id, &emails)?;
    let reader = Reader::open(&xdg::cache_dir()?)?;
    let thread = reader.thread_of(&key)?;
    if thread.is_empty() {
        return Err(format!("the Cache holds no Thread {id}").into());
    }

    Ok((key.account().clone(), thread))
}

/// The Account and the Thread of the Message file `path`, which the TUI
/// names `<account>/<body key>.eml`: a body key alone is shared across
/// Accounts whose servers number alike.
fn opened(path: &Path) -> Result<(String, Vec<ThreadMessage>), Box<dyn Error>> {
    let wrong = || format!("{} is no Message file", path.display());
    let key = path
        .file_stem()
        .and_then(|stem| stem.to_str()?.parse().ok())
        .map(BodyKey::from_stored)
        .ok_or_else(wrong)?;
    let named = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .ok_or_else(wrong)?;
    let account = accounts::emails()?
        .into_iter()
        .find(|email| email.eq_ignore_ascii_case(named))
        .ok_or_else(|| format!("{named} is no Account set up"))?;
    let reader = Reader::open(&xdg::cache_dir()?)?;
    let kept = reader
        .message(&account, key)?
        .ok_or_else(|| format!("the Cache holds no Message for {}", path.display()))?;
    let thread = reader.thread_of(&Key::new(account.clone(), kept.thread, kept.root, key))?;
    Ok((account, thread))
}

/// The Thread `answering` names in `account`.
fn answered(account: &str, answering: &Answering) -> Result<Vec<ThreadMessage>, Box<dyn Error>> {
    let reader = Reader::open(&xdg::cache_dir()?)?;
    let thread = reader
        .thread(account, answering)?
        .ok_or("the Draft answers no Message the Cache knows")?;
    if thread.is_empty() {
        return Err("the Message the Draft answers is not in the Cache".into());
    }

    Ok(thread)
}

/// Each Message as a header block, a blank line and its body, a blank line
/// between Messages.
fn text(thread: &[ThreadMessage], rows: &[Row<'_>]) -> String {
    let each: Vec<String> = thread
        .iter()
        .zip(rows)
        .map(|(kept, row)| {
            format!(
                "From: {}\nRecipients: {}\nDate: {}\n\n{}\n",
                row.from,
                row.recipients,
                header_date::date(&kept.received).as_str(),
                row.body.as_deref().unwrap_or(MISSING)
            )
        })
        .collect();
    each.join("\n")
}
