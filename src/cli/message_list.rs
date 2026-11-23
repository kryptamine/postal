use std::error::Error;

use crate::{
    account::xdg,
    cache::Reader,
    cli::accounts,
    cli::answer::{Answer, Answers},
    cli::args::{LIMIT, MessageList},
    cli::thread_rows::{self, ThreadRows},
    domain::folder::{Folder, INBOX},
};

/// `postal message list`: the newest Threads of a Folder, by default every
/// Account's Inbox.
impl Answers for MessageList {
    fn answer(&self) -> Result<Answer, Box<dyn Error>> {
        let wanted = accounts::named(self.account.as_deref())?;
        let reader = Reader::open(&xdg::cache_dir()?)?;
        let folders = reader.folders()?;
        let mut places = Vec::new();
        for account in &wanted {
            let name = (self.folder.as_ref())
                .map_or(Some(INBOX), |folder| named(&folders, account, folder));
            places.extend(name.map(|name| (account.as_str(), name)));
        }

        if places.is_empty()
            && let Some(folder) = &self.folder
        {
            return Err(format!("no Account has a Folder {folder}").into());
        }

        let mut threads = ThreadRows::new(self.limit.unwrap_or(LIMIT));
        reader.listed(&places, |listed| threads.visit(listed))?;
        let rows = threads.into_rows();
        Ok(Answer::new(&rows, thread_rows::table(&rows))?)
    }
}

/// The server's name of `account`'s Folder that `wanted` names: by that
/// name first, else as people name it.
fn named<'a>(folders: &'a [Folder<String>], account: &str, wanted: &str) -> Option<&'a str> {
    let mine = || folders.iter().filter(|f| f.account == *account);
    mine()
        .find(|f| f.name.as_str() == wanted)
        .or_else(|| mine().find(|f| f.answers_to(wanted)))
        .map(|f| f.name.as_str())
}
