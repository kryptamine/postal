use chrono::{DateTime, FixedOffset};

/// One Message as the Message List knows it.
pub struct Message {
    pub uid: u32,
    /// Where the Message's body is cached, from `body_key::body_key`.
    pub body_key: i64,
    /// The server's `INTERNALDATE`: both the sort key and the date column.
    pub received: DateTime<FixedOffset>,
    pub unread: bool,
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
}
