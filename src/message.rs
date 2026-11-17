use chrono::{DateTime, FixedOffset};

/// One row of the Message List.
pub struct Message {
    pub uid: u32,
    /// The server's `INTERNALDATE`: both the sort key and the date column.
    pub received: DateTime<FixedOffset>,
    pub unread: bool,
    pub sender: String,
    pub subject: String,
}
