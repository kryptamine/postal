use crate::{flag::Flag, uid::Uid};

/// One change of a flag the server has not acknowledged yet.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FlagChange {
    pub uid: Uid,
    pub flag: Flag,
    /// Whether the flag is added, rather than removed.
    pub added: bool,
}
