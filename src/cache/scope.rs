use super::{Cache, Error};
use crate::{
    domain::folder_name::FolderName,
    domain::per_account::{AccountId, PerAccount},
    domain::query::Query,
    domain::shown::Shown,
};

/// The Folders a Search reads, and the buffers its window reuses, so
/// reading the window again allocates only to grow them (ADR 0001).
pub struct Scope {
    /// Each Account's Folders the Query reads, by which copy of a Message
    /// wins: the shown one first, then in Folder order (ADR 0010).
    pub(super) accounts: Vec<(AccountId, Vec<FolderName>)>,
    pub(super) lone: String,
    pub(super) threads: String,
    pub(super) roots: String,
    /// The rank of the Folder each Message the window read comes from.
    pub(super) ranks: Vec<usize>,
}

impl Scope {
    /// Reads anew the Folders `query` reads, `shown` naming each Account's
    /// that wins; the buffers stay.
    pub fn reload(
        &mut self,
        caches: &PerAccount<Cache>,
        shown: &Shown,
        query: &Query,
    ) -> Result<(), Error> {
        self.accounts.clear();
        for (account, cache) in caches.iter() {
            let mut folders = cache.folders()?;
            folders.retain(|f| query.reads(cache.email(), f));
            folders.sort_by_key(|f| Some(&f.name) != shown.of(account));
            let names = folders.into_iter().map(|f| f.name).collect();
            self.accounts.push((account, names));
        }

        Ok(())
    }

    /// Each Folder read, by its Account's address and its name.
    pub(super) fn places<'a>(&'a self, caches: &'a PerAccount<Cache>) -> Vec<(&'a str, &'a str)> {
        let each = |(account, folders): &'a (AccountId, Vec<FolderName>)| {
            let email = caches[*account].email();
            folders.iter().map(move |folder| (email, folder.as_str()))
        };
        self.accounts.iter().flat_map(each).collect()
    }

    /// Nothing, for a Search before its first read.
    pub const fn empty() -> Self {
        Self {
            accounts: Vec::new(),
            lone: String::new(),
            threads: String::new(),
            roots: String::new(),
            ranks: Vec::new(),
        }
    }
}

/// The rank of the Folder named `mailbox` among `folders`; `None` when a
/// Search leaves it out.
pub(super) fn rank(folders: &[FolderName], mailbox: &str) -> Option<usize> {
    folders.iter().position(|f| f.as_str() == mailbox)
}
