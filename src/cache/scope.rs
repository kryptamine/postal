use super::{Cache, Error};
use crate::{
    domain::folder::Role,
    domain::folder_name::FolderName,
    domain::per_account::{AccountId, PerAccount},
    domain::shown::Shown,
};

/// The Folders a Search reads, and the buffers its window reuses, so
/// reading the window again allocates only to grow them (ADR 0001).
pub struct Scope {
    /// Each Account's Folders but Trash and Spam, by which copy of a
    /// Message wins: the shown one first, then in Folder order (ADR 0010).
    pub(super) accounts: Vec<(AccountId, Vec<FolderName>)>,
    pub(super) lone: String,
    pub(super) threads: String,
    pub(super) roots: String,
    /// The rank of the Folder each Message the window read comes from.
    pub(super) ranks: Vec<usize>,
}

impl Scope {
    /// Reads the Folders anew, `shown` naming each Account's that wins;
    /// the buffers stay.
    pub fn reload(&mut self, caches: &PerAccount<Cache>, shown: &Shown) -> Result<(), Error> {
        self.accounts.clear();
        for (account, cache) in caches.iter() {
            let mut folders = cache.folders()?;
            folders.retain(|f| !matches!(f.role, Some(Role::Trash | Role::Spam)));
            folders.sort_by_key(|f| Some(&f.name) != shown.of(account));
            let names = folders.into_iter().map(|f| f.name).collect();
            self.accounts.push((account, names));
        }

        Ok(())
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
