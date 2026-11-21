use super::Cache;
use crate::domain::folder_name::FolderName;

/// What a Folder's rows are stored under: the address of its Account and
/// its name on the server.
#[derive(Clone, Copy)]
pub struct FolderKey<'a> {
    pub account: &'a str,
    pub folder: &'a str,
}

impl Cache {
    /// What `folder`'s rows of this Account are stored under.
    pub(super) fn key<'a>(&'a self, folder: &'a FolderName) -> FolderKey<'a> {
        FolderKey {
            account: self.email(),
            folder: folder.as_str(),
        }
    }
}
