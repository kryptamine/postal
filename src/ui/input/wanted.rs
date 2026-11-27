use crate::{domain::message_ref::MessageRef, domain::per_account::AccountId};

/// What a picked attachment is for.
#[derive(Clone, Copy)]
pub enum Purpose {
    /// In the program the desktop picks for it.
    Open,
    /// Into the download directory.
    Save,
}

/// One attachment picked, and what for.
pub struct Wanted {
    pub account: AccountId,
    pub message: MessageRef,
    /// Its place among its Message's attachments.
    pub index: usize,
    pub name: String,
    pub size: u64,
    pub purpose: Purpose,
}
