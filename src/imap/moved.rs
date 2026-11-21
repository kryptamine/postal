use crate::uid::Uid;

/// Where the server put one moved Message, as its `COPYUID` names it.
#[derive(Clone, Copy)]
pub struct Moved {
    pub from: Uid,
    pub to: Uid,
}
