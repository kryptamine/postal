use std::sync::Arc;

use crate::message::Message;

/// A Reply waiting on the bodies of its Thread the Cache lacks, so that
/// `postal thread show` reads the whole correspondence.
pub struct AwaitedReply {
    pub message: Message,
    /// Reply All.
    pub all: bool,
    /// The Folder and UID of each body still to come.
    pub missing: Vec<(Arc<str>, u32)>,
}
