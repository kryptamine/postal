use crate::{
    compose::recipients::Recipients, ui::outside::open_outside::Outside,
    ui::outside::wanted_attachments::Purpose,
};

#[derive(Clone, Copy)]
pub enum Action {
    Move(Move),
    Scroll(Scroll),
    /// The selected Thread full screen.
    Read,
    /// From the full-screen Thread to the list.
    Back,
    /// The cursor to the next or previous Message of the Thread.
    Step(Direction),
    /// Folds or unfolds the Message at the cursor.
    Fold,
    /// The selected Message outside the TUI; in Reading, the one at the
    /// cursor.
    Open(Outside),
    /// Mark Unread: the selected Thread's newest Message, or in Reading the
    /// one at the cursor.
    MarkUnread,
    /// Delete: the selected Thread's Messages in their Folders, or in
    /// Reading the one at the cursor.
    Delete,
    /// Archive, as Delete takes Messages to Trash.
    Archive,
    /// Move, as Delete, to a Folder the Folder Picker picks.
    MoveTo,
    /// Undo of the latest move or Draft discard not yet taken back.
    Undo,
    /// A new Draft in `$EDITOR`.
    Compose,
    /// A Reply's Draft in `$EDITOR`.
    Reply(Recipients),
    /// A Forward's Draft of the selected Message, or in Reading of the one
    /// at the cursor, in `$EDITOR`.
    Forward,
    /// Hint mode on the links in the Preview window.
    Links,
    /// Hint mode on the attachments of the selected Message, or in Reading
    /// of the one at the cursor.
    Attachments(Purpose),
    /// The other View of HTML mail: rendered or text.
    SwitchView,
    /// Shows the Quote of the selected Message, or in Reading of the one
    /// at the cursor, or hides it.
    SwitchQuoted,
    Help,
    /// Types the Query in the hint line: a new Search, or the one under way.
    Search,
    Type(char),
    /// The Query's last character deleted.
    Erase,
    /// The menu's next or previous suggestion written into the Query.
    Complete(Direction),
    /// The suggestion written in taken, and the menu suggesting anew.
    Accept,
    /// The narrowed list, with the Query kept.
    KeepQuery,
    /// The full list again: while the Query is typed, on the Message
    /// selected before the Search, else on the one selected now.
    EndSearch,
    /// The login flow again, while signed out.
    SignIn,
    Settings,
    Folders,
    Quit,
}

#[derive(Clone, Copy)]
pub enum Move {
    Down,
    Up,
    First,
    Last,
}

#[derive(Clone, Copy)]
pub struct Scroll {
    pub direction: Direction,
    pub by: By,
}

#[derive(Clone, Copy)]
pub enum Direction {
    Down,
    Up,
}

#[derive(Clone, Copy)]
pub enum By {
    Line,
    HalfPage,
    /// As far as the Preview goes.
    End,
}
