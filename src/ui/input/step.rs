use std::{path::PathBuf, sync::Arc};

use crate::{
    account::config::{IconChoice, NotifyChoice},
    compose::recipients::Recipients,
    domain::folder::Folder,
    domain::per_account::AccountId,
    domain::shown::Shown,
    domain::thread_id::ThreadId,
    ui::exit::Exit,
    ui::outside::open_outside::Outside,
    ui::outside::wanted_attachments::{Purpose, Wanted},
};

/// What the event loop does after one input event.
pub enum Step {
    Stay,
    Links,
    Steer(Steer),
    Draft(Drafting),
    Reply(Recipients),
    Forward,
    Browse(Arc<str>),
    /// Hint mode on the attachment line, to pick one for this.
    Attachments(Purpose),
    Attachment(Wanted),
    Open(Outside),
    LocalDrafts,
    Settings,
    /// The editor on the signature of the Account at this address.
    Signature(String),
    Icons(IconChoice),
    Notify(NotifyChoice),
    Exit(Exit),
}

/// What changes the Folder shown or the Messages, the sync connection asked
/// to follow.
pub enum Steer {
    Folders,
    Show(Shown),
    Search,
    /// A character typed into the Query of the Search under way.
    Type(char),
    /// The Query's last character erased.
    Erase,
    /// Reading opened, on the Gmail Thread with this ID if it has one, of
    /// this Account: its Messages in other Folders are asked for.
    Read(Option<(AccountId, ThreadId)>),
    MarkUnread,
    Delete,
    Archive,
    /// The Folder Picker opened to move Messages to the Folder picked.
    Pick,
    MoveTo(Folder),
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
