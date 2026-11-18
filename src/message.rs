use chrono::{DateTime, FixedOffset};

/// One row of the Message List.
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
}
