use serde::{Deserialize, Serialize};

use crate::domain::folder_name::FolderName;
use crate::domain::uid::Uid;

/// A Message on the server: its UID in the Folder named `folder`.
/// Written as a `[folder, uid]` pair, as a Draft's notes always held it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "(FolderName, Uid)", into = "(FolderName, Uid)")]
pub struct MessageRef {
    pub folder: FolderName,
    pub uid: Uid,
}

impl From<(FolderName, Uid)> for MessageRef {
    fn from((folder, uid): (FolderName, Uid)) -> Self {
        Self { folder, uid }
    }
}

impl From<MessageRef> for (FolderName, Uid) {
    fn from(message: MessageRef) -> Self {
        (message.folder, message.uid)
    }
}
