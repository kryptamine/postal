use super::Reported;
use crate::{
    domain::message_flags::MessageFlags, domain::new_message::NewMessage, domain::uid::Uid,
};

/// What one resync learnt, written to the Cache in one transaction.
pub struct Changes {
    pub reported: Reported,
    /// Drops every cached row first, as after a UIDVALIDITY change.
    pub replace: bool,
    pub new: Vec<NewMessage>,
    pub expunged: Vec<Uid>,
    /// A UID not in the Cache is ignored.
    pub flags: Vec<(Uid, MessageFlags)>,
}

impl Changes {
    /// Every cached row of the Folder gives way to `new`.
    pub const fn replacing(reported: Reported, new: Vec<NewMessage>) -> Self {
        Self {
            reported,
            replace: true,
            new,
            expunged: Vec::new(),
            flags: Vec::new(),
        }
    }

    pub const fn update(
        reported: Reported,
        new: Vec<NewMessage>,
        expunged: Vec<Uid>,
        flags: Vec<(Uid, MessageFlags)>,
    ) -> Self {
        Self {
            reported,
            replace: false,
            new,
            expunged,
            flags,
        }
    }
}
