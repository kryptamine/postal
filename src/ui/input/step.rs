use std::{path::PathBuf, sync::Arc};

use crate::{
    account::config::{AvatarChoice, ContactsChoice, IconChoice, NotifyChoice},
    compose::recipients::Recipients,
    domain::folder::Folder,
    domain::per_account::AccountId,
    domain::shown::Shown,
    domain::thread_id::ThreadId,
    ui::exit::Exit,
    ui::input::keymap::Direction,
    ui::input::keymap::Outside,
    ui::input::wanted::{Purpose, Wanted},
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
    Original,
    LocalDrafts,
    Settings,
    /// The editor on the signature of the Account at this address.
    Signature(String),
    Live(Live),
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
    /// The menu's next suggestion written into the Query, or its previous.
    Complete(Direction),
    /// The suggestion written in taken, for the menu to suggest anew.
    Accept,
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
    Refresh,
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

/// A setting change that takes effect on the running screen at once.
#[derive(Clone, Copy)]
pub enum Live {
    Icons(IconChoice),
    Avatars(AvatarChoice),
    Notify(NotifyChoice),
    /// Where Search finds Correspondents besides mail, read at once when
    /// turned on so macOS asks for access then.
    Contacts(ContactsChoice),
}
