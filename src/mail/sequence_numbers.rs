use std::sync::Arc;

use crate::domain::uid::Uid;

/// The ordered UID list: which cached Message a sequence number from the
/// IDLE connection names. The Cache holds the newest Messages without gaps,
/// so a sequence number past the older ones indexes straight into it.
pub struct SequenceNumbers {
    /// The Folder whose `SELECT` numbered them.
    folder: Arc<str>,
    /// Messages on the server older than every cached one.
    older: u32,
    /// The cached UIDs, ascending.
    uids: Vec<Uid>,
}

impl SequenceNumbers {
    /// `exists` is the count `SELECT` saw, before any Message at or past
    /// `uid_next` arrived; `None` counts every cached UID as seen by it.
    pub fn new(folder: Arc<str>, exists: u32, uid_next: Option<Uid>, uids: Vec<Uid>) -> Self {
        let examined = uids
            .iter()
            .filter(|&&uid| uid_next.is_none_or(|next| uid < next))
            .count();
        let examined = u32::try_from(examined).unwrap_or(u32::MAX);
        Self {
            folder,
            older: exists.saturating_sub(examined),
            uids,
        }
    }

    pub fn folder(&self) -> &str {
        &self.folder
    }

    pub fn uid(&self, sequence: u32) -> Option<Uid> {
        let index = sequence.checked_sub(self.older + 1)?;
        self.uids.get(usize::try_from(index).ok()?).copied()
    }

    /// Whether `EXISTS` counts Messages the Cache does not hold yet.
    pub fn grew(&self, exists: u32) -> bool {
        usize::try_from(exists.saturating_sub(self.older)).is_ok_and(|n| n > self.uids.len())
    }

    /// Takes the expunged Message out, renumbering the ones after it;
    /// returns its UID, or `None` when it was older than the Cache.
    pub fn expunge(&mut self, sequence: u32) -> Option<Uid> {
        if sequence <= self.older {
            self.older = self.older.saturating_sub(1);
            return None;
        }

        let index = usize::try_from(sequence - self.older - 1).ok()?;
        (index < self.uids.len()).then(|| self.uids.remove(index))
    }

    /// The server's Message count as announced so far.
    pub fn exists(&self) -> u32 {
        self.older + u32::try_from(self.uids.len()).unwrap_or(u32::MAX)
    }

    pub fn oldest(&self) -> Option<Uid> {
        self.uids.first().copied()
    }

    /// The lowest UID a new Message can have.
    pub fn next_uid(&self) -> Uid {
        self.uids.last().map_or(Uid::new(1), |last| last.next())
    }

    /// Appends new Messages' UIDs, each above every cached one.
    pub fn extend(&mut self, uids: impl IntoIterator<Item = Uid>) {
        self.uids.extend(uids);
    }
}
