use crate::{new_message::NewMessage, sync_state::SyncState};

/// What one resync learnt, written to the Cache in one transaction.
pub struct Changes {
    pub state: SyncState,
    /// The server's count over the whole Inbox.
    pub unread: usize,
    /// Drops every cached row first, as after a UIDVALIDITY change.
    pub replace: bool,
    pub new: Vec<NewMessage>,
    pub expunged: Vec<u32>,
    /// Unread per UID; a UID not in the Cache is ignored.
    pub flags: Vec<(u32, bool)>,
}
