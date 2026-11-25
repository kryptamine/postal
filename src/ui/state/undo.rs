use std::path::PathBuf;

use crate::{
    compose::draft_notes::DraftNotes, domain::per_account::AccountId, domain::thread::Key,
};

/// What an Undo takes back.
pub enum Undo {
    /// A Delete, an Archive or a Move: each Account's batch in the Cache,
    /// and the Thread selected when it was made.
    Move(Vec<(AccountId, i64)>, Option<Key>),
    /// A discarded Draft: its file, what the file held, the address it goes
    /// out from and what it answers.
    Discard(PathBuf, String, DraftNotes),
    /// A Send held in the Outbox, at this path while it waits.
    Send(PathBuf),
}
