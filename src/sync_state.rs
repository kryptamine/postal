use crate::uid::Uid;

/// What a resync saw of the Inbox, so the next one fetches only what changed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SyncState {
    pub uid_validity: u32,
    /// `None` when the server sent no `UIDNEXT`.
    pub uid_next: Option<Uid>,
    /// `None` when the server has no CONDSTORE for the Inbox.
    pub highest_modseq: Option<u64>,
}
