use crate::{message_flags::MessageFlags, new_message::NewMessage, sync_state::SyncState};

/// What one resync learnt, written to the Cache in one transaction.
pub struct Changes {
    pub state: SyncState,
    /// The server's Message count of the Folder.
    pub exists: u32,
    /// The server's count over the whole Inbox.
    pub unread: usize,
    /// Drops every cached row first, as after a UIDVALIDITY change.
    pub replace: bool,
    pub new: Vec<NewMessage>,
    pub expunged: Vec<u32>,
    /// A UID not in the Cache is ignored.
    pub flags: Vec<(u32, MessageFlags)>,
}
