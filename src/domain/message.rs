use chrono::{DateTime, FixedOffset};

use crate::{
    domain::attachment::Attachments, domain::body_key::BodyKey, domain::folder_name::FolderName,
    domain::message_ref::MessageRef, domain::per_account::AccountId, domain::root_id::RootId,
    domain::thread_id::ThreadId, domain::uid::Uid,
};

/// One Message as the Message List knows it.
#[derive(Clone)]
pub struct Message<A = AccountId> {
    pub uid: Uid,
    /// Where the Message's body is cached.
    pub body_key: BodyKey,
    /// The server's `INTERNALDATE`: both the sort key and the date column.
    pub received: DateTime<FixedOffset>,
    pub unread: bool,
    pub answered: bool,
    pub sender: String,
    /// The first `From` address as a header shows it, `Name <address>`.
    pub from: String,
    /// `To`, then `Cc`, as `from` is shown, joined by commas.
    pub to: String,
    pub subject: String,
    /// Empty until the second fetch phase reaches the Message, and for good
    /// when it has no text part.
    pub snippet: String,
    pub attachments: Attachments,
    /// Gmail's `X-GM-THRID`; `None` elsewhere.
    pub thread: Option<ThreadId>,
    /// Elsewhere, its Thread by `References`. `None` on Gmail, and when it
    /// names no `Message-ID`: then the Message is a Thread of its own.
    pub root: Option<RootId>,
    /// Sent from the Account's address: the Message List names its sender `me`.
    pub from_me: bool,
    /// The Folder `uid` is of. `None` for a Message found only with its
    /// Thread, whose body comes with the Thread, and for one not yet cached.
    pub folder: Option<FolderName>,
    /// `()` for a Message read from the server, which knows no Account: only
    /// the Cache names one.
    pub account: A,
}

impl<A> Message<A> {
    /// `None` for a Message found only with its Thread.
    pub fn on_server(&self) -> Option<MessageRef> {
        let folder = self.folder.clone()?;
        Some(MessageRef {
            folder,
            uid: self.uid,
        })
    }
}

impl Message {
    /// A slot for a Message of `account` to be read into.
    pub fn blank(account: AccountId) -> Self {
        Self {
            uid: Uid::new(0),
            body_key: BodyKey::from_stored(0),
            received: DateTime::default(),
            unread: false,
            answered: false,
            sender: String::new(),
            from: String::new(),
            to: String::new(),
            subject: String::new(),
            snippet: String::new(),
            attachments: Attachments::default(),
            thread: None,
            root: None,
            from_me: false,
            folder: None,
            account,
        }
    }
}
