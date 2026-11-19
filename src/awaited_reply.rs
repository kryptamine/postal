use crate::message::Message;

/// A Reply waiting on the body of the Message it answers, which the Cache
/// had evicted.
pub struct AwaitedReply {
    pub message: Message,
    /// Reply All.
    pub all: bool,
}
