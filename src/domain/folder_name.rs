use std::{
    fmt,
    sync::{Arc, LazyLock},
};

use serde::{Deserialize, Serialize};

/// The Folder every run starts in, as IMAP names it in any case.
pub const INBOX: &str = "INBOX";

/// A Folder's name as the server gives it, modified UTF-7 and nested labels
/// in full, which every command names it by; not the path people read.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FolderName(Arc<str>);

impl FolderName {
    /// The Inbox, as IMAP names it in any case.
    pub fn inbox() -> &'static Self {
        static NAME: LazyLock<FolderName> = LazyLock::new(|| FolderName::from(INBOX));
        &NAME
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// For the IMAP library, which takes any `AsRef<str>` as a name.
impl AsRef<str> for FolderName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for FolderName {
    fn from(name: &str) -> Self {
        Self(name.into())
    }
}

impl From<String> for FolderName {
    fn from(name: String) -> Self {
        Self(name.into())
    }
}

impl fmt::Debug for FolderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for FolderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
