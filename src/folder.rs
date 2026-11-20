use std::{cmp::Ordering, sync::Arc};

/// The Folder every run starts in, as IMAP names it in any case.
pub const INBOX: &str = "INBOX";

/// What a special-use Folder is for (RFC 6154), in the order the Folder
/// Picker lists them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Role {
    /// Every Account's Inbox in one Message List, never on the server.
    AllInboxes,
    Inbox,
    Starred,
    Sent,
    Drafts,
    /// Postal's own Folder of Drafts and the Outbox, never on the server.
    LocalDrafts,
    All,
    Spam,
    Trash,
}

impl Role {
    /// The server's roles: the Cache never stores `LocalDrafts`.
    const ALL: [Self; 7] = [
        Self::Inbox,
        Self::Starred,
        Self::Sent,
        Self::Drafts,
        Self::All,
        Self::Spam,
        Self::Trash,
    ];

    /// The name the Folder is shown under, whatever the server calls it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::AllInboxes => "All Inboxes",
            Self::Inbox => "Inbox",
            Self::Starred => "Starred",
            Self::Sent => "Sent",
            Self::Drafts => "Drafts",
            Self::All => "All Mail",
            Self::Spam => "Spam",
            Self::Trash => "Trash",
            Self::LocalDrafts => "Local Drafts",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.label() == label)
    }
}

/// One Folder on the server, as the Folder Picker lists it.
#[derive(Clone, Debug)]
pub struct Folder {
    /// The Account's place in this run; a name repeats across Accounts.
    pub account: usize,
    /// As the server names it, in every command; nested labels in full.
    pub name: Arc<str>,
    /// `name` decoded from modified UTF-7, for people to read.
    pub path: Arc<str>,
    pub role: Option<Role>,
    /// `None` until a resync or a `STATUS` counts it.
    pub unread: Option<usize>,
}

impl Folder {
    pub fn inbox(account: usize) -> Self {
        Self::new(account, INBOX, None)
    }

    /// Local Drafts, as the Folder Picker lists it among the server's.
    pub fn local_drafts() -> Self {
        Self::new(0, Role::LocalDrafts.label(), Some(Role::LocalDrafts))
    }

    pub fn all_inboxes() -> Self {
        Self::new(0, Role::AllInboxes.label(), Some(Role::AllInboxes))
    }

    pub fn is_all_inboxes(&self) -> bool {
        self.role == Some(Role::AllInboxes)
    }

    /// Folders are known by Account and name; those never on the server by
    /// their role.
    pub fn same(&self, other: &Self) -> bool {
        let local = |f: &Self| matches!(f.role, Some(Role::LocalDrafts | Role::AllInboxes));
        match (local(self), local(other)) {
            (true, true) => self.role == other.role,
            (false, false) => self.account == other.account && self.name == other.name,
            _ => false,
        }
    }

    /// The Inbox is known by its name alone (RFC 3501).
    pub fn new(account: usize, name: &str, role: Option<Role>) -> Self {
        let inbox = name.eq_ignore_ascii_case(INBOX);
        let name: Arc<str> = if inbox { INBOX.into() } else { name.into() };
        Self {
            account,
            path: crate::modified_utf7::decode(&name).into(),
            name,
            role: if inbox { Some(Role::Inbox) } else { role },
            unread: None,
        }
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
