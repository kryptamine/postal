use crate::domain::message::Message;

/// The Messages a Search reads from the Cache.
pub struct Inbox {
    /// Newest first.
    pub messages: Vec<Message>,
    /// Messages of their Threads found in other Folders, oldest first.
    pub elsewhere: Vec<Message>,
}
