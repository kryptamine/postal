use crate::domain::uid::Uid;

/// Where the server put one moved Message, as its `COPYUID` names it.
pub struct Moved {
    pub from: Uid,
    pub to: Uid,
}
