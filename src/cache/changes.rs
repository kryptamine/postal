use crate::{
    domain::message_flags::MessageFlags, domain::new_message::NewMessage,
    domain::sync_state::SyncState, domain::uid::Uid,
};

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
    pub expunged: Vec<Uid>,
    /// A UID not in the Cache is ignored.
    pub flags: Vec<(Uid, MessageFlags)>,
}
