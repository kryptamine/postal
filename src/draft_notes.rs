use crate::answering::Answering;

/// The notes of a Draft: the address of the Account it goes out from, the
/// first Account's when `None`, and for a Reply, what it answers.
#[derive(Default)]
pub struct DraftNotes {
    pub account: Option<String>,
    pub answering: Option<Answering>,
}
