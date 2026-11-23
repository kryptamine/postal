use std::error::Error;

use crate::{
    account::xdg,
    cache::Reader,
    cli::accounts,
    cli::answer::Answer,
    cli::args::{LIMIT, MessageList},
    cli::thread_rows::{self, ThreadRows},
};

/// `postal message list`: the newest Threads of a Folder, by default every
/// Account's Inbox.
pub fn answer(list: &MessageList) -> Result<Answer, Box<dyn Error>> {
    let emails = accounts::emails()?;
    let wanted: Vec<&String> = match &list.account {
        Some(account) => {
            let account = account.to_lowercase();
            vec![
                emails
                    .iter()
                    .find(|&email| *email == account)
                    .ok_or_else(|| format!("no Account {account} is set up"))?,
            ]
        }
        None => emails.iter().collect(),
    };

    let reader = Reader::open(&xdg::cache_dir()?)?;
    let mut places = Vec::new();
    for account in wanted {
        let name = match &list.folder {
            Some(folder) => reader.folder_named(account, folder)?,
            None => Some("INBOX".to_owned()),
        };

        places.extend(name.map(|name| (account.as_str(), name)));
    }

    if places.is_empty()
        && let Some(folder) = &list.folder
    {
        return Err(format!("no Account has a Folder {folder}").into());
    }

    let places: Vec<(&str, &str)> = places.iter().map(|(a, n)| (*a, n.as_str())).collect();
    let mut threads = ThreadRows::new(list.limit.unwrap_or(LIMIT));
    reader.listed(&places, |listed| threads.visit(listed))?;
    let rows = threads.into_rows();
    Ok(Answer::new(&rows, thread_rows::table(&rows))?)
}
