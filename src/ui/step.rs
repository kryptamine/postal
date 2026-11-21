use std::sync::Arc;

use crate::{
    account::config::IconChoice, domain::per_account::AccountId, domain::shown::Shown,
    domain::thread_id::ThreadId, ui::open_outside::Outside,
};

/// What the event loop does after one input event.
pub enum Step {
    Stay,
    Links,
    Folders,
    Show(Shown),
    Search,
    /// Reading opened, on the Gmail Thread with this ID if it has one, of
    /// this Account: its Messages in other Folders are asked for.
    Read(Option<(AccountId, ThreadId)>),
    MarkUnread,
    Delete,
    Undo,
    Browse(Arc<str>),
    Open(Outside),
    LocalDrafts,
    Compose,
    /// Reply, or Reply All when true.
    Reply(bool),
    /// The shown Draft in the editor again.
    Edit,
    Send,
    /// The sign-in of this Account, signed out.
    SignIn(AccountId),
    Settings,
    Icons(IconChoice),
    AddAccount,
    Unlink(String),
    Reload,
    Quit,
}
