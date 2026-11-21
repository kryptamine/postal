use crate::{message::Message, message_ref::MessageRef};

/// A Reply waiting on the bodies of its Thread the Cache lacks, so that
/// `postal thread show` reads the whole correspondence.
pub struct AwaitedReply {
    pub message: Message,
    /// Reply All.
    pub all: bool,
    /// The Folder and UID of each body still to come.
    pub missing: Vec<MessageRef>,
}
