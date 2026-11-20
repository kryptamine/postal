use std::{io, path::PathBuf};

use crate::{draft, draft_file, notes::Notes, outbox::Outbox};

/// One Draft or Outbox Message as Local Drafts lists it.
pub struct LocalDraft {
    pub path: PathBuf,
    /// In the Outbox, so shown read-only.
    pub queued: bool,
    pub to: String,
    pub subject: String,
    /// The address of the Account it goes out from, as its notes name it.
    pub account: Option<String>,
}

impl LocalDraft {
    fn new(path: PathBuf, queued: bool, (text, account): (&str, Option<String>)) -> Self {
        Self {
            account,
            path,
            queued,
            to: draft::header(text, "To").unwrap_or_default(),
            subject: draft::header(text, "Subject").unwrap_or_default(),
        }
    }
}

/// Every Draft and every Outbox Message not yet accepted, newest first. A
/// file that cannot be read is left out: a Send may be moving it.
pub fn list(outbox: &Outbox) -> io::Result<Vec<LocalDraft>> {
    let mut rows = Vec::new();
    for path in draft_file::listed(&draft_file::dir()?)? {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let account = Notes::read(&path)
                    .ok()
                    .flatten()
                    .and_then(|notes| notes.account);
                rows.push(LocalDraft::new(path, false, (&text, account)));
            }
            Err(error) => tracing::warn!("reading {} failed: {error}", path.display()),
        }
    }

    for queued in outbox.queued()?.into_iter().filter(|q| !q.sent) {
        match Notes::read(&queued.path) {
            Ok(notes) => {
                let notes = notes.unwrap_or_default();
                let text = notes.draft.unwrap_or_default();
                rows.push(LocalDraft::new(queued.path, true, (&text, notes.account)));
            }
            Err(error) => tracing::warn!("reading {} failed: {error}", queued.path.display()),
        }
    }

    // A file's name starts with the time its Draft was made.
    rows.sort_by(|a, b| b.path.file_name().cmp(&a.path.file_name()));
    Ok(rows)
}
