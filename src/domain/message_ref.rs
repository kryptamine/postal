use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::domain::uid::Uid;

/// A Message on the server: its UID in the Folder named `folder`.
/// Written as a `[folder, uid]` pair, as a Draft's notes always held it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "(Arc<str>, Uid)", into = "(Arc<str>, Uid)")]
pub struct MessageRef {
    pub folder: Arc<str>,
    pub uid: Uid,
}

impl From<(Arc<str>, Uid)> for MessageRef {
    fn from((folder, uid): (Arc<str>, Uid)) -> Self {
        Self { folder, uid }
    }
}

impl From<MessageRef> for (Arc<str>, Uid) {
    fn from(message: MessageRef) -> Self {
        (message.folder, message.uid)
    }
}
