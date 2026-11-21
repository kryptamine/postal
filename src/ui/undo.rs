use std::path::PathBuf;

use crate::{
    compose::draft_notes::DraftNotes, domain::per_account::AccountId, domain::thread::Key,
};

/// What an Undo takes back.
pub enum Undo {
    /// A Delete: its Account, its batch in the Cache, and the Thread
    /// selected when it was made.
    Delete(AccountId, i64, Option<Key>),
    /// A discarded Draft: its file, what the file held, the address it goes
    /// out from and what it answers.
    Discard(PathBuf, String, DraftNotes),
}
