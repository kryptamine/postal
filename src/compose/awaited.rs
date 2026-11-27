use crate::{
    compose::awaited_forward::AwaitedForward, compose::awaited_reply::AwaitedReply,
    domain::message::Message,
};

/// What a Draft waits on the fetch connection for. One at a time: each
/// would open its own Draft over the other.
pub enum Awaited {
    Reply(AwaitedReply),
    Forward(AwaitedForward),
}

impl Awaited {
    /// The Message answered or forwarded.
    pub const fn message(&self) -> &Message {
        match self {
            Self::Reply(awaited) => &awaited.message,
            Self::Forward(awaited) => &awaited.message,
        }
    }
}
