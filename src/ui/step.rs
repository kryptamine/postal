use std::sync::Arc;

use crate::{
    account::config::IconChoice, domain::per_account::AccountId, domain::shown::Shown,
    domain::thread_id::ThreadId, ui::exit::Exit, ui::open_outside::Outside,
};

/// What the event loop does after one input event.
pub enum Step {
    Stay,
    Links,
    Steer(Steer),
    Draft(Drafting),
    /// Reply, or Reply All when true.
    Reply(bool),
    Browse(Arc<str>),
    Open(Outside),
    LocalDrafts,
    Settings,
    Icons(IconChoice),
    Exit(Exit),
}

/// What changes the Folder shown or the Messages, the sync connection asked
/// to follow.
pub enum Steer {
    Folders,
    Show(Shown),
    Search,
    /// Reading opened, on the Gmail Thread with this ID if it has one, of
    /// this Account: its Messages in other Folders are asked for.
    Read(Option<(AccountId, ThreadId)>),
    MarkUnread,
    Delete,
    Undo,
}

/// What a Draft goes through in the editor and the Outbox.
#[derive(Clone, Copy)]
pub enum Drafting {
    Compose,
    /// The shown Draft in the editor again.
    Edit,
    Send,
}
