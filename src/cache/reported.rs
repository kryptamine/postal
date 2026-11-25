use crate::domain::sync_state::SyncState;

/// What the server last reported of a Folder, as its `mailbox` row keeps it.
pub struct Reported {
    pub state: SyncState,
    /// The server's Message count of the Folder.
    pub exists: u32,
    /// The server's count over the whole Inbox.
    pub unread: usize,
}

impl Reported {
    pub const fn new(state: SyncState, exists: u32, unread: usize) -> Self {
        Self {
            state,
            exists,
            unread,
        }
    }
}
