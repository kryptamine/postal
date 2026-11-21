use std::{error::Error, io};

use chrono::Utc;
use serde::Serialize;
use unicode_width::UnicodeWidthStr;

use crate::{
    cache::Reader,
    cli::CorrespondentList,
    command_output,
    config::Config,
    correspondent::{self, Correspondent},
    notes::Notes,
    xdg,
};

/// One Correspondent of `--json`, whose fields only grow (ADR 0017).
#[derive(Serialize)]
struct Row<'a> {
    address: &'a str,
    name: Option<&'a str>,
    /// As a Draft's `To` holds it.
    mailbox: String,
    account: &'a str,
    written: bool,
}

/// `postal correspondent list`: every Correspondent the Cache knows, best
/// first, the Account the arguments name first.
pub fn run(list: &CorrespondentList) -> Result<(), Box<dyn Error>> {
    let configs = Config::load(&xdg::config_dir()?.join("config.toml"))?;
    let emails: Vec<&str> = configs.iter().map(|c| c.email.as_str()).collect();
    let seen = Reader::open(&xdg::cache_dir()?)?.correspondence()?;
    let first = first_account(list)?.or_else(|| emails.first().map(|&e| e.to_owned()));
    let first = first.unwrap_or_default();
    let ranked = correspondent::ranked(seen, (&emails, &first), Utc::now());

    let text = if list.json {
        json(&ranked)?
    } else {
        table(&ranked, emails.len() > 1)
    };

    Ok(command_output::print(&text)?)
}

/// The address `--draft`'s notes or `--account` name; `None` for neither.
fn first_account(list: &CorrespondentList) -> io::Result<Option<String>> {
    if let Some(draft) = &list.draft {
        return Ok(Notes::read(draft)?.and_then(|notes| notes.account));
    }

    Ok(list.account.clone())
}

fn json(ranked: &[Correspondent]) -> serde_json::Result<String> {
    let rows: Vec<Row<'_>> = ranked
        .iter()
        .map(|c| Row {
            address: &c.address,
            name: c.name.as_deref(),
            mailbox: c.mailbox(),
            account: &c.account,
            written: c.written,
        })
        .collect();
    let mut text = serde_json::to_string(&rows)?;
    text.push('\n');
    Ok(text)
}

/// A line each, with the Account that knows it in a column of its own when
/// there are several.
fn table(ranked: &[Correspondent], accounts: bool) -> String {
    let mailboxes: Vec<String> = ranked.iter().map(Correspondent::mailbox).collect();
    let width = mailboxes.iter().map(|m| m.width()).max().unwrap_or(0);
    let mut text = String::new();
    for (mailbox, correspondent) in mailboxes.iter().zip(ranked) {
        text.push_str(mailbox);
        if accounts {
            let gap = width - mailbox.width() + 2;
            text.push_str(&" ".repeat(gap));
            text.push_str(&correspondent.account);
        }

        text.push('\n');
    }

    text
}
