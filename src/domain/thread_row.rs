use crate::{domain::thread::Key, domain::uid::Uid};

/// One Thread of the shown Folder in the Message List, by what orders it.
#[derive(Clone, Copy)]
pub struct ThreadRow {
    pub key: Key,
    /// Its newest Message's arrival in seconds, then UID: rows run newest
    /// first by both.
    pub newest: (i64, Uid),
}
