use crate::folder::{Folder, INBOX, Role};

/// What the Message List shows and the Folder Picker lists: a server
/// Folder, or one of the two Postal keeps of its own.
#[derive(Clone, Debug)]
pub enum Shown {
    /// Every Account's Inbox in one Message List.
    AllInboxes {
        unread: Option<usize>,
    },
    Folder(Folder),
    /// Postal's own Drafts and the Outbox, opened over the Message List.
    LocalDrafts,
}

impl Shown {
    pub const fn all_inboxes() -> Self {
        Self::AllInboxes { unread: None }
    }

    /// The server Folder of `account` this shows: each Account's Inbox in
    /// All Inboxes, else a Folder's own for its Account.
    pub fn of(&self, account: usize) -> Option<&str> {
        match self {
            Self::AllInboxes { .. } => Some(INBOX),
            Self::Folder(folder) => (account == folder.account).then_some(&*folder.name),
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
    pub fn is(&self, account: usize, name: &str) -> bool {
        self.folder()
            .is_some_and(|folder| folder.account == account && *folder.name == *name)
    }

    /// A Folder is known by Account and name; an Unread count never counts.
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

    pub const fn unread(&self) -> Option<usize> {
        match self {
            Self::AllInboxes { unread } => *unread,
            Self::Folder(folder) => folder.unread,
            Self::LocalDrafts => None,
        }
    }
}
