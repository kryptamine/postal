use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::mode::Mode;

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
    /// The selected Message in `$PAGER`; in Reading, the one at the cursor.
    Page,
    /// Mark Unread: the selected Thread's newest Message, or in Reading the
    /// one at the cursor.
    MarkUnread,
    /// Delete: the selected Thread's Messages in their Folders, or in
    /// Reading the one at the cursor.
    Delete,
    /// Undo of the latest Delete or Draft discard not yet taken back.
    Undo,
    /// A new Draft in `$EDITOR`.
    Compose,
    /// A Reply's Draft in `$EDITOR`, to everyone the Message went to when
    /// `all`.
    Reply {
        all: bool,
    },
    /// Hint mode on the links in the Preview window.
    Links,
    /// The other View of HTML mail: rendered or text.
    SwitchView,
    Help,
    /// Types the Query in the hint line: a new Search, or the one under way.
    Search,
    Type(char),
    /// The Query's last character deleted.
    Erase,
    /// The narrowed list, with the Query kept.
    KeepQuery,
    /// The full list again: on the Message selected before the Search when
    /// `restore`, else on the one selected now.
    EndSearch {
        restore: bool,
    },
    /// The login flow again, while signed out.
    SignIn,
    Folders,
    Suspend,
    Quit,
}

/// What a key does while a Draft's preview is open.
#[derive(Clone, Copy)]
pub enum DraftKey {
    Send,
    Edit,
    /// Closes the preview, the Draft kept in Local Drafts.
    Keep,
    Discard,
    Help,
    Quit,
}

/// What a key does in Local Drafts.
#[derive(Clone, Copy)]
pub enum LocalKey {
    Down,
    Up,
    /// A Draft's preview, or an Outbox Message read-only.
    Open,
    Discard,
    Undo,
    Close,
    Help,
    Quit,
}

/// What a key does while an Outbox Message is shown read-only.
#[derive(Clone, Copy)]
pub enum QueuedKey {
    Close,
    Help,
    Quit,
}

/// What a key does while the Folder Picker is open.
#[derive(Clone, Copy)]
pub enum Pick {
    Type(char),
    Erase,
    Down,
    Up,
    /// Shows the selected Folder.
    Choose,
    Close,
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

/// One row of the `?` popup.
pub struct Binding {
    pub group: &'static str,
    pub modes: &'static [Mode],
    /// Listed, and active, only while signed out.
    pub signed_out: bool,
    pub keys: &'static str,
    pub aliases: &'static str,
    pub description: &'static str,
    pub hint: Option<Hint>,
}

pub struct Hint {
    pub keys: &'static str,
    pub description: &'static str,
}

const LIST: &[Mode] = &[Mode::List];
const READING: &[Mode] = &[Mode::Reading];
const BOTH: &[Mode] = &[Mode::List, Mode::Reading];
const DRAFT: &[Mode] = &[Mode::Draft];
const LOCAL: &[Mode] = &[Mode::LocalDrafts];
const ANY: &[Mode] = &[
    Mode::List,
    Mode::Reading,
    Mode::Draft,
    Mode::LocalDrafts,
    Mode::Queued,
];

/// The keymap as the hint line and the `?` popup show it, in their order.
/// `action` below reads the same keys; change the two together.
const BINDINGS: [Binding; 41] = [
    Binding {
        group: "Navigate",
        modes: LIST,
        signed_out: false,
        keys: "j / k",
        aliases: "↓ / ↑",
        description: "next / previous Message",
        hint: Some(Hint {
            keys: "j/k",
            description: "move",
        }),
    },
    Binding {
        group: "Navigate",
        modes: LIST,
        signed_out: false,
        keys: "g / G",
        aliases: "Home / End",
        description: "first / last Message",
        hint: None,
    },
    Binding {
        group: "Read",
        modes: READING,
        signed_out: false,
        keys: "j / k",
        aliases: "↓ / ↑",
        description: "scroll a line",
        hint: Some(Hint {
            keys: "j/k",
            description: "scroll",
        }),
    },
    Binding {
        group: "Read",
        modes: READING,
        signed_out: false,
        keys: "g / G",
        aliases: "Home / End",
        description: "top / bottom of the Thread",
        hint: None,
    },
    Binding {
        group: "Read",
        modes: READING,
        signed_out: false,
        keys: "^d / ^u",
        aliases: "PgDn / PgUp",
        description: "scroll half a page",
        hint: Some(Hint {
            keys: "^d/^u",
            description: "page",
        }),
    },
    Binding {
        group: "Read",
        modes: READING,
        signed_out: false,
        keys: "n / p",
        aliases: "",
        description: "next / previous Message",
        hint: Some(Hint {
            keys: "n/p",
            description: "next",
        }),
    },
    Binding {
        group: "Read",
        modes: READING,
        signed_out: false,
        keys: "Enter",
        aliases: "Space",
        description: "fold / unfold the Message",
        hint: Some(Hint {
            keys: "enter",
            description: "fold",
        }),
    },
    Binding {
        group: "Read",
        modes: READING,
        signed_out: false,
        keys: "|",
        aliases: "",
        description: "open the Message in $PAGER",
        hint: Some(Hint {
            keys: "|",
            description: "pager",
        }),
    },
    Binding {
        group: "Read",
        modes: READING,
        signed_out: false,
        keys: "q / Esc",
        aliases: "h / ←",
        description: "back to the list",
        hint: Some(Hint {
            keys: "q",
            description: "back",
        }),
    },
    Binding {
        group: "Message",
        modes: LIST,
        signed_out: false,
        keys: "^d / ^u",
        aliases: "PgDn / PgUp",
        description: "scroll the Preview half a page",
        hint: Some(Hint {
            keys: "^d/^u",
            description: "scroll",
        }),
    },
    Binding {
        group: "Message",
        modes: LIST,
        signed_out: false,
        keys: "Enter",
        aliases: "l",
        description: "read the Message full screen",
        hint: Some(Hint {
            keys: "enter",
            description: "read",
        }),
    },
    Binding {
        group: "Message",
        modes: LIST,
        signed_out: false,
        keys: "|",
        aliases: "",
        description: "open the Message in $PAGER",
        hint: None,
    },
    Binding {
        group: "Message",
        modes: BOTH,
        signed_out: false,
        keys: "U",
        aliases: "",
        description: "mark the Message Unread",
        hint: None,
    },
    Binding {
        group: "Message",
        modes: BOTH,
        signed_out: false,
        keys: "Backspace",
        aliases: "",
        description: "delete: move to Trash",
        hint: None,
    },
    Binding {
        group: "Message",
        modes: BOTH,
        signed_out: false,
        keys: "u",
        aliases: "",
        description: "undo a delete or discard",
        hint: None,
    },
    Binding {
        group: "Message",
        modes: BOTH,
        signed_out: false,
        keys: "c",
        aliases: "",
        description: "compose a new Message",
        hint: None,
    },
    Binding {
        group: "Message",
        modes: BOTH,
        signed_out: false,
        keys: "r",
        aliases: "",
        description: "reply to the sender",
        hint: None,
    },
    Binding {
        group: "Message",
        modes: BOTH,
        signed_out: false,
        keys: "R",
        aliases: "",
        description: "reply to all",
        hint: None,
    },
    Binding {
        group: "Links",
        modes: BOTH,
        signed_out: false,
        keys: "o",
        aliases: "",
        description: "label the links in the Preview",
        hint: Some(Hint {
            keys: "o",
            description: "links",
        }),
    },
    Binding {
        group: "View",
        modes: BOTH,
        signed_out: false,
        keys: "v",
        aliases: "",
        description: "rendered / text view of HTML",
        hint: None,
    },
    Binding {
        group: "App",
        modes: BOTH,
        signed_out: true,
        keys: "L",
        aliases: "",
        description: "sign in again",
        hint: Some(Hint {
            keys: "L",
            description: "sign in",
        }),
    },
    Binding {
        group: "App",
        modes: ANY,
        signed_out: false,
        keys: "?",
        aliases: "",
        description: "this help",
        hint: None,
    },
    Binding {
        group: "App",
        modes: LIST,
        signed_out: false,
        keys: "q",
        aliases: "^c",
        description: "quit",
        hint: Some(Hint {
            keys: "q",
            description: "quit",
        }),
    },
    Binding {
        group: "App",
        modes: &[Mode::Reading, Mode::Draft, Mode::LocalDrafts, Mode::Queued],
        signed_out: false,
        keys: "^c",
        aliases: "",
        description: "quit",
        hint: None,
    },
    Binding {
        group: "App",
        modes: BOTH,
        signed_out: false,
        keys: "^z",
        aliases: "",
        description: "suspend to the shell",
        hint: None,
    },
    Binding {
        group: "App",
        modes: READING,
        signed_out: false,
        keys: "f",
        aliases: "",
        description: "pick the Folder to show",
        hint: None,
    },
    Binding {
        group: "Search",
        modes: LIST,
        signed_out: false,
        keys: "/",
        aliases: "",
        description: "search, or edit the Query",
        hint: Some(Hint {
            keys: "/",
            description: "search",
        }),
    },
    Binding {
        group: "Search",
        modes: LIST,
        signed_out: false,
        keys: "^j / ^k",
        aliases: "↓ / ↑",
        description: "move, while typing the Query",
        hint: None,
    },
    Binding {
        group: "Search",
        modes: LIST,
        signed_out: false,
        keys: "Enter",
        aliases: "",
        description: "keep the Query",
        hint: None,
    },
    Binding {
        group: "Search",
        modes: LIST,
        signed_out: false,
        keys: "Esc",
        aliases: "",
        description: "end the Search",
        hint: None,
    },
    Binding {
        group: "Folders",
        modes: LIST,
        signed_out: false,
        keys: "f",
        aliases: "",
        description: "pick the Folder to show",
        hint: Some(Hint {
            keys: "f",
            description: "folders",
        }),
    },
    Binding {
        group: "Draft",
        modes: DRAFT,
        signed_out: false,
        keys: "y",
        aliases: "",
        description: "send the Draft",
        hint: Some(Hint {
            keys: "y",
            description: "send",
        }),
    },
    Binding {
        group: "Draft",
        modes: DRAFT,
        signed_out: false,
        keys: "e",
        aliases: "",
        description: "edit it again",
        hint: Some(Hint {
            keys: "e",
            description: "edit",
        }),
    },
    Binding {
        group: "Draft",
        modes: DRAFT,
        signed_out: false,
        keys: "q",
        aliases: "",
        description: "discard it; u brings it back",
        hint: Some(Hint {
            keys: "q",
            description: "discard",
        }),
    },
    Binding {
        group: "Draft",
        modes: DRAFT,
        signed_out: false,
        keys: "d",
        aliases: "",
        description: "keep it in Local Drafts",
        hint: Some(Hint {
            keys: "d",
            description: "keep",
        }),
    },
    Binding {
        group: "Local Drafts",
        modes: LOCAL,
        signed_out: false,
        keys: "j / k",
        aliases: "↓ / ↑",
        description: "next / previous",
        hint: Some(Hint {
            keys: "j/k",
            description: "move",
        }),
    },
    Binding {
        group: "Local Drafts",
        modes: LOCAL,
        signed_out: false,
        keys: "Enter",
        aliases: "",
        description: "open the Draft or the Outbox Message",
        hint: Some(Hint {
            keys: "Enter",
            description: "open",
        }),
    },
    Binding {
        group: "Local Drafts",
        modes: LOCAL,
        signed_out: false,
        keys: "Backspace",
        aliases: "",
        description: "discard the Draft",
        hint: None,
    },
    Binding {
        group: "Local Drafts",
        modes: LOCAL,
        signed_out: false,
        keys: "u",
        aliases: "",
        description: "undo a discard",
        hint: None,
    },
    Binding {
        group: "Local Drafts",
        modes: LOCAL,
        signed_out: false,
        keys: "q",
        aliases: "Esc",
        description: "back to the Message List",
        hint: Some(Hint {
            keys: "q",
            description: "back",
        }),
    },
    Binding {
        group: "Outbox",
        modes: &[Mode::Queued],
        signed_out: false,
        keys: "q",
        aliases: "Esc",
        description: "back to Local Drafts",
        hint: Some(Hint {
            keys: "q",
            description: "back",
        }),
    },
];

/// The bindings listed in `mode` now, in `BINDINGS` order.
pub fn bindings(mode: Mode, signed_out: bool) -> impl Iterator<Item = &'static Binding> + Clone {
    BINDINGS
        .iter()
        .filter(move |b| b.modes.contains(&mode) && (signed_out || !b.signed_out))
}

/// The hint line's last entry, kept however narrow the terminal.
pub const HELP_HINT: Hint = Hint {
    keys: "?:",
    description: "help",
};

const fn scroll(direction: Direction, by: By) -> Action {
    Action::Scroll(Scroll { direction, by })
}

pub const fn action(key: KeyEvent, mode: Mode) -> Option<Action> {
    if !pressed(key) {
        return None;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        // `^j` arrives as `\n`, which crossterm reads as `^j`, not Enter.
        return match (mode, key.code) {
            (_, KeyCode::Char('c')) => Some(Action::Quit),
            (_, KeyCode::Char('d')) => Some(scroll(Direction::Down, By::HalfPage)),
            (_, KeyCode::Char('u')) => Some(scroll(Direction::Up, By::HalfPage)),
            (_, KeyCode::Char('z')) => Some(Action::Suspend),
            (Mode::Query, KeyCode::Char('j')) => Some(Action::Move(Move::Down)),
            (Mode::Query, KeyCode::Char('k')) => Some(Action::Move(Move::Up)),
            _ => None,
        };
    }

    match (mode, key.code) {
        (Mode::Query, KeyCode::Enter) => Some(Action::KeepQuery),
        (Mode::Query, KeyCode::Esc) => Some(Action::EndSearch { restore: true }),
        (Mode::Query, KeyCode::Backspace) => Some(Action::Erase),
        // Every character is the Query's, so these arms come first.
        (Mode::Query, KeyCode::Char(c)) => Some(Action::Type(c)),
        (_, KeyCode::PageDown) => Some(scroll(Direction::Down, By::HalfPage)),
        (_, KeyCode::PageUp) => Some(scroll(Direction::Up, By::HalfPage)),
        (_, KeyCode::Char('o')) => Some(Action::Links),
        (_, KeyCode::Char('v')) => Some(Action::SwitchView),
        (_, KeyCode::Char('|')) => Some(Action::Page),
        (_, KeyCode::Char('?')) => Some(Action::Help),
        (_, KeyCode::Char('L')) => Some(Action::SignIn),
        (_, KeyCode::Char('f')) => Some(Action::Folders),
        (Mode::List | Mode::Query, KeyCode::Char('j') | KeyCode::Down) => {
            Some(Action::Move(Move::Down))
        }
        (Mode::List | Mode::Query, KeyCode::Char('k') | KeyCode::Up) => {
            Some(Action::Move(Move::Up))
        }
        (Mode::List, KeyCode::Char('g') | KeyCode::Home) => Some(Action::Move(Move::First)),
        (Mode::List, KeyCode::Char('G') | KeyCode::End) => Some(Action::Move(Move::Last)),
        (Mode::List, KeyCode::Enter | KeyCode::Char('l')) => Some(Action::Read),
        (Mode::List, KeyCode::Char('q')) => Some(Action::Quit),
        (Mode::List, KeyCode::Char('/')) => Some(Action::Search),
        (Mode::List, KeyCode::Esc) => Some(Action::EndSearch { restore: false }),
        (Mode::Reading, KeyCode::Char('j') | KeyCode::Down) => {
            Some(scroll(Direction::Down, By::Line))
        }
        (Mode::Reading, KeyCode::Char('k') | KeyCode::Up) => Some(scroll(Direction::Up, By::Line)),
        (Mode::Reading, KeyCode::Char('g') | KeyCode::Home) => Some(scroll(Direction::Up, By::End)),
        (Mode::Reading, KeyCode::Char('G') | KeyCode::End) => {
            Some(scroll(Direction::Down, By::End))
        }
        (Mode::Reading, KeyCode::Char('n')) => Some(Action::Step(Direction::Down)),
        (Mode::Reading, KeyCode::Char('p')) => Some(Action::Step(Direction::Up)),
        (Mode::Reading, KeyCode::Enter | KeyCode::Char(' ')) => Some(Action::Fold),
        (Mode::Reading, KeyCode::Char('q' | 'h') | KeyCode::Esc | KeyCode::Left) => {
            Some(Action::Back)
        }
        (Mode::List | Mode::Reading, code) => on_message(code),
        _ => None,
    }
}

/// What `code` does to a Message, in the list and in Reading alike.
const fn on_message(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Char('U') => Some(Action::MarkUnread),
        KeyCode::Backspace => Some(Action::Delete),
        KeyCode::Char('u') => Some(Action::Undo),
        KeyCode::Char('c') => Some(Action::Compose),
        KeyCode::Char('r') => Some(Action::Reply { all: false }),
        KeyCode::Char('R') => Some(Action::Reply { all: true }),
        _ => None,
    }
}

/// What `key` does in a Draft's preview, which takes them all.
pub const fn draft(key: KeyEvent) -> Option<DraftKey> {
    if !pressed(key) {
        return None;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Some(DraftKey::Quit),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Char('y') => Some(DraftKey::Send),
        KeyCode::Char('e') => Some(DraftKey::Edit),
        KeyCode::Char('d') => Some(DraftKey::Keep),
        KeyCode::Char('q') => Some(DraftKey::Discard),
        KeyCode::Char('?') => Some(DraftKey::Help),
        _ => None,
    }
}

/// What `key` does in Local Drafts, which takes them all.
pub const fn local(key: KeyEvent) -> Option<LocalKey> {
    if !pressed(key) {
        return None;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Some(LocalKey::Quit),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(LocalKey::Down),
        KeyCode::Char('k') | KeyCode::Up => Some(LocalKey::Up),
        KeyCode::Enter => Some(LocalKey::Open),
        KeyCode::Backspace => Some(LocalKey::Discard),
        KeyCode::Char('u') => Some(LocalKey::Undo),
        KeyCode::Char('q') | KeyCode::Esc => Some(LocalKey::Close),
        KeyCode::Char('?') => Some(LocalKey::Help),
        _ => None,
    }
}

/// What `key` does while an Outbox Message is shown, which takes them all.
pub const fn queued(key: KeyEvent) -> Option<QueuedKey> {
    if !pressed(key) {
        return None;
    }

    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(QueuedKey::Quit)
        }
        _ if key.modifiers.contains(KeyModifiers::CONTROL) => None,
        KeyCode::Char('q') | KeyCode::Esc => Some(QueuedKey::Close),
        KeyCode::Char('?') => Some(QueuedKey::Help),
        _ => None,
    }
}

/// What `key` does in the Folder Picker; every character is the Query's.
pub const fn pick(key: KeyEvent) -> Option<Pick> {
    if !pressed(key) {
        return None;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Some(Pick::Quit),
            KeyCode::Char('n') => Some(Pick::Down),
            KeyCode::Char('p') => Some(Pick::Up),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Enter => Some(Pick::Choose),
        KeyCode::Esc => Some(Pick::Close),
        KeyCode::Backspace => Some(Pick::Erase),
        KeyCode::Down => Some(Pick::Down),
        KeyCode::Up => Some(Pick::Up),
        KeyCode::Char(c) => Some(Pick::Type(c)),
        _ => None,
    }
}

/// Whether `key` closes the `?` popup; while it is open, no other key acts.
pub const fn closes_help(key: KeyEvent) -> bool {
    pressed(key)
        && !key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('?' | 'q') | KeyCode::Esc)
}

pub const fn pressed(key: KeyEvent) -> bool {
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}
