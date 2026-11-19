use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// What a Reply's Draft carries beside its file: the headers that put it in
/// the Thread, and the Message it answers.
#[derive(Clone, Serialize, Deserialize)]
pub struct Answering {
    /// The answered Message's `Message-ID`, when it has one.
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    /// Where `\Answered` goes once the Reply is sent; `None` for a Message
    /// found only with its Thread.
    pub original: Option<(Arc<str>, u32)>,
}
