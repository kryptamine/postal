use std::path::PathBuf;

use crate::{answering::Answering, thread::Key};

/// What an Undo takes back.
pub enum Undo {
    /// A Delete: its batch in the Cache, and the Thread selected when it
    /// was made.
    Delete(i64, Option<Key>),
    /// A discarded Draft: its file, what the file held, and what it answers.
    Discard(PathBuf, String, Option<Answering>),
}
