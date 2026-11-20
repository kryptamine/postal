use std::sync::Arc;

use chrono::{DateTime, FixedOffset};

/// One Message as the Message List knows it.
#[derive(Clone)]
pub struct Message {
    pub uid: u32,
    /// Where the Message's body is cached, from `body_key::body_key`.
    pub body_key: i64,
    /// The server's `INTERNALDATE`: both the sort key and the date column.
    pub received: DateTime<FixedOffset>,
    pub unread: bool,
    pub answered: bool,
    pub sender: String,
    pub subject: String,
    /// Empty until the second fetch phase reaches the Message, and for good
    /// when it has no text part.
    pub snippet: String,
    /// Gmail's `X-GM-THRID`; `None` elsewhere, where the Message is a Thread
    /// of its own.
    pub thread: Option<u64>,
    /// Sent from the Account's address: the Message List names its sender `me`.
    pub from_me: bool,
    /// The Folder `uid` is of. `None` for a Message found only with its
    /// Thread, whose body comes with the Thread, and for one not yet cached.
    pub folder: Option<Arc<str>>,
    /// The Account's place in this run, as the Cache read the row; 0 for
    /// one not read from it yet.
    pub account: usize,
}
