use crate::{
    compose::recipients::Recipients, domain::message::Message, domain::message_ref::MessageRef,
};

/// A Reply waiting on the bodies of its Thread the Cache lacks, so that
/// `postal thread show` reads the whole correspondence.
pub struct AwaitedReply {
    pub message: Message,
    pub to: Recipients,
    /// The Folder and UID of each body still to come.
    pub missing: Vec<MessageRef>,
}
