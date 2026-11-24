use crate::domain::{
    counts::Counts,
    folder::{Folder, Role},
    folder_name::FolderName,
    per_account::AccountId,
};

/// What the Message List shows and the Folder Picker lists: a server
/// Folder, or one of the two Postal keeps of its own.
#[derive(Clone, Debug)]
pub enum Shown {
    /// Every Account's Inbox in one Message List.
    AllInboxes {
        counts: Option<Counts>,
    },
    Folder(Folder),
    /// Postal's own Drafts and the Outbox, opened over the Message List.
    LocalDrafts,
}

impl Shown {
    pub const fn all_inboxes() -> Self {
        Self::AllInboxes { counts: None }
    }

    /// The server Folder of `account` this shows: each Account's Inbox in
    /// All Inboxes, else a Folder's own for its Account.
    pub fn of(&self, account: AccountId) -> Option<&FolderName> {
        match self {
            Self::AllInboxes { .. } => Some(FolderName::inbox()),
            Self::Folder(folder) => (account == folder.account).then_some(&folder.name),
            Self::LocalDrafts => None,
        }
    }

    pub const fn folder(&self) -> Option<&Folder> {
        match self {
            Self::Folder(folder) => Some(folder),
            _ => None,
        }
    }

    pub fn role(&self) -> Option<Role> {
        self.folder().and_then(|folder| folder.role)
    }

    /// Whether this is the server Folder `name` of `account` itself.
    pub fn is(&self, account: AccountId, name: &FolderName) -> bool {
        self.folder()
            .is_some_and(|folder| folder.account == account && folder.name == *name)
    }

    /// A Folder is known by Account and name; its counts never count.
    pub fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::AllInboxes { .. }, Self::AllInboxes { .. })
            | (Self::LocalDrafts, Self::LocalDrafts) => true,
            (Self::Folder(folder), other) => other.is(folder.account, &folder.name),
            _ => false,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::AllInboxes { .. } => "All Inboxes",
            Self::Folder(folder) => folder.label(),
            Self::LocalDrafts => "Local Drafts",
        }
    }

    pub const fn counts(&self) -> Option<Counts> {
        match self {
            Self::AllInboxes { counts } => *counts,
            Self::Folder(folder) => folder.counts,
            Self::LocalDrafts => None,
        }
    }
}
