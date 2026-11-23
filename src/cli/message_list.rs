use std::error::Error;

use crate::{
    account::xdg,
    cache::Reader,
    cli::accounts,
    cli::answer::Answer,
    cli::args::{LIMIT, MessageList},
    cli::thread_rows::{self, ThreadRows},
    domain::folder::{Folder, INBOX},
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
    let folders = reader.folders()?;
    let mut places = Vec::new();
    for account in wanted {
        let name =
            (list.folder.as_ref()).map_or(Some(INBOX), |folder| named(&folders, account, folder));
        places.extend(name.map(|name| (account.as_str(), name)));
    }

    if places.is_empty()
        && let Some(folder) = &list.folder
    {
        return Err(format!("no Account has a Folder {folder}").into());
    }

    let mut threads = ThreadRows::new(list.limit.unwrap_or(LIMIT));
    reader.listed(&places, |listed| threads.visit(listed))?;
    let rows = threads.into_rows();
    Ok(Answer::new(&rows, thread_rows::table(&rows))?)
}

/// The server's name of `account`'s Folder that `wanted` names: by that
/// name, else by the name people read or its role's label, in any case.
fn named<'a>(folders: &'a [Folder<String>], account: &str, wanted: &str) -> Option<&'a str> {
    let mine = || folders.iter().filter(|f| f.account == *account);
    mine()
        .find(|f| f.name.as_str() == wanted)
        .or_else(|| {
            mine().find(|f| {
                f.path.to_lowercase() == wanted.to_lowercase()
                    || f.role
                        .is_some_and(|role| role.label().eq_ignore_ascii_case(wanted))
            })
        })
        .map(|f| f.name.as_str())
}
