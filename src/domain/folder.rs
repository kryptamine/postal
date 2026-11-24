use std::{cmp::Ordering, sync::Arc};

use crate::domain::counts::Counts;
use crate::domain::folder_name::FolderName;
use crate::domain::per_account::AccountId;

/// The Folder every run starts in, as IMAP names it in any case.
pub const INBOX: &str = "INBOX";

/// What a special-use Folder is for (RFC 6154), in the order the Folder
/// Picker lists them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Role {
    Inbox,
    Starred,
    Sent,
    Drafts,
    Archive,
    All,
    Spam,
    Trash,
}

impl Role {
    const ALL: [Self; 8] = [
        Self::Inbox,
        Self::Starred,
        Self::Sent,
        Self::Drafts,
        Self::Archive,
        Self::All,
        Self::Spam,
        Self::Trash,
    ];

    /// The name the Folder is shown under, whatever the server calls it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Inbox => "Inbox",
            Self::Starred => "Starred",
            Self::Sent => "Sent",
            Self::Drafts => "Drafts",
            Self::Archive => "Archive",
            Self::All => "All Mail",
            Self::Spam => "Spam",
            Self::Trash => "Trash",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.label() == label)
    }
}

/// One Folder on the server, as the Folder Picker lists it.
#[derive(Clone, Debug)]
pub struct Folder<A = AccountId> {
    /// A name repeats across Accounts. `()` for a Folder the server lists,
    /// which knows no Account: only the Cache names one.
    pub account: A,
    /// As the server names it, in every command; nested labels in full.
    pub name: FolderName,
    /// `name` decoded from modified UTF-7, for people to read.
    pub path: Arc<str>,
    pub role: Option<Role>,
    /// `None` until a resync or a `STATUS` counts it.
    pub counts: Option<Counts>,
}

impl Folder {
    pub fn inbox(account: AccountId) -> Self {
        Self::new(account, INBOX, None)
    }
}

impl<A> Folder<A> {
    /// The Inbox is known by its name alone (RFC 3501).
    pub fn new(account: A, name: &str, role: Option<Role>) -> Self {
        let inbox = name.eq_ignore_ascii_case(INBOX);
        let name: FolderName = if inbox { INBOX.into() } else { name.into() };
        let path = crate::domain::modified_utf7::decode(name.as_str()).into();
        Self {
            account,
            path,
            name,
            role: if inbox { Some(Role::Inbox) } else { role },
            counts: None,
        }
    }

    /// Whether `wanted` names it as people do: its path or its role's label,
    /// in any case.
    pub fn answers_to(&self, wanted: &str) -> bool {
        self.path.to_lowercase() == wanted.to_lowercase()
            || self
                .role
                .is_some_and(|role| role.label().eq_ignore_ascii_case(wanted))
    }

    pub fn label(&self) -> &str {
        match self.role {
            Some(role) => role.label(),
            None => &self.path,
        }
    }

    /// Special-use Folders first, in `Role` order, then the rest by name.
    pub fn order(&self, other: &Self) -> Ordering {
        let rank = |folder: &Self| (folder.role.is_none(), folder.role);
        rank(self).cmp(&rank(other)).then_with(|| {
            let lower = |folder: &Self| folder.path.to_lowercase();
            lower(self).cmp(&lower(other))
        })
    }
}
