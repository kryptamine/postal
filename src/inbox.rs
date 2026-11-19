use crate::message::Message;

/// What the server holds for the Message List.
pub struct Inbox {
    /// The newest Messages, in server order.
    pub messages: Vec<Message>,
    /// Messages of their Threads found in other Folders, oldest first.
    pub elsewhere: Vec<Message>,
}
