use std::path::PathBuf;

use crate::thread::Key;

/// What an Undo takes back.
pub enum Undo {
    /// A Delete: its batch in the Cache, and the Thread selected when it
    /// was made.
    Delete(i64, Option<Key>),
    /// A discarded Draft: its file, and what the file held.
    Discard(PathBuf, String),
}
