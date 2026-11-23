use crate::{domain::body_key::BodyKey, domain::root_id::RootId, domain::thread_id::ThreadId};

/// What names a Thread within one Account, as `domain::thread::Key` does
/// across them.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreadOf {
    Gmail(ThreadId),
    Root(RootId),
    /// A Message with neither, a Thread of its own, wherever it is.
    Lone(BodyKey),
}

impl ThreadOf {
    /// Gmail's Thread first, then the root of `References`, else the body.
    pub const fn of(thread: Option<ThreadId>, root: Option<RootId>, body: BodyKey) -> Self {
        match (thread, root) {
            (Some(id), _) => Self::Gmail(id),
            (None, Some(root)) => Self::Root(root),
            (None, None) => Self::Lone(body),
        }
    }
}
