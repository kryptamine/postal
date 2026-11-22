use std::{path::PathBuf, sync::Arc};

use crate::{
    account::config::IconChoice, compose::recipients::Recipients, domain::per_account::AccountId,
    domain::shown::Shown, domain::thread_id::ThreadId, ui::exit::Exit,
    ui::outside::open_outside::Outside,
};

/// What the event loop does after one input event.
pub enum Step {
    Stay,
    Links,
    Steer(Steer),
    Draft(Drafting),
    Reply(Recipients),
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

/// What a Draft goes through: the editor, the Outbox, Local Drafts or a
/// discard.
pub enum Drafting {
    Compose,
    /// The shown Draft in the editor again.
    Edit,
    Send,
    /// Closes the shown Draft's preview, its file kept for Local Drafts.
    Keep,
    /// Deletes the shown Draft's file, for an Undo to write back.
    Discard,
    /// The Draft at this path, as from its preview.
    DiscardFile(PathBuf),
}
