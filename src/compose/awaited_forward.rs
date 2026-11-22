use crate::{domain::message::Message, domain::message_ref::MessageRef};

/// A Forward waiting on what the Cache lacks of its Message: the body,
/// and each attachment, which is never cached.
pub struct AwaitedForward {
    pub message: Message,
    pub on_server: MessageRef,
    pub body_missing: bool,
    /// Each attachment's bytes once fetched, in the Message's order.
    pub attachments: Vec<Option<Vec<u8>>>,
}

impl AwaitedForward {
    pub fn complete(&self) -> bool {
        !self.body_missing && self.attachments.iter().all(Option::is_some)
    }
}
