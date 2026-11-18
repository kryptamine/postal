use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::mode::Mode;

#[derive(Clone, Copy)]
pub enum Action {
    Move(Move),
    Scroll(Scroll),
    /// The selected Message full screen.
    Read,
    /// From the full-screen Message to the list.
    Back,
    /// The selected Message in `$PAGER`.
    Page,
    /// Hint mode on the links in the Preview window.
    Links,
    Help,
    /// The login flow again, while signed out.
    SignIn,
    Suspend,
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

/// The keymap as the hint line and the `?` popup show it, in their order.
/// `action` below reads the same keys; change the two together.
const BINDINGS: [Binding; 16] = [
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
        description: "top / bottom of the Message",
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
        modes: BOTH,
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
        modes: READING,
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
        return match key.code {
            KeyCode::Char('c') => Some(Action::Quit),
            KeyCode::Char('d') => Some(scroll(Direction::Down, By::HalfPage)),
            KeyCode::Char('u') => Some(scroll(Direction::Up, By::HalfPage)),
            KeyCode::Char('z') => Some(Action::Suspend),
            _ => None,
        };
    }

    match (mode, key.code) {
        (_, KeyCode::PageDown) => Some(scroll(Direction::Down, By::HalfPage)),
        (_, KeyCode::PageUp) => Some(scroll(Direction::Up, By::HalfPage)),
        (_, KeyCode::Char('o')) => Some(Action::Links),
        (_, KeyCode::Char('|')) => Some(Action::Page),
        (_, KeyCode::Char('?')) => Some(Action::Help),
        (_, KeyCode::Char('L')) => Some(Action::SignIn),
        (Mode::List, KeyCode::Char('j') | KeyCode::Down) => Some(Action::Move(Move::Down)),
        (Mode::List, KeyCode::Char('k') | KeyCode::Up) => Some(Action::Move(Move::Up)),
        (Mode::List, KeyCode::Char('g') | KeyCode::Home) => Some(Action::Move(Move::First)),
        (Mode::List, KeyCode::Char('G') | KeyCode::End) => Some(Action::Move(Move::Last)),
        (Mode::List, KeyCode::Enter | KeyCode::Char('l')) => Some(Action::Read),
        (Mode::List, KeyCode::Char('q')) => Some(Action::Quit),
        (Mode::Reading, KeyCode::Char('j') | KeyCode::Down) => {
            Some(scroll(Direction::Down, By::Line))
        }
        (Mode::Reading, KeyCode::Char('k') | KeyCode::Up) => Some(scroll(Direction::Up, By::Line)),
        (Mode::Reading, KeyCode::Char('g') | KeyCode::Home) => Some(scroll(Direction::Up, By::End)),
        (Mode::Reading, KeyCode::Char('G') | KeyCode::End) => {
            Some(scroll(Direction::Down, By::End))
        }
        (Mode::Reading, KeyCode::Char('q' | 'h') | KeyCode::Esc | KeyCode::Left) => {
            Some(Action::Back)
        }
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
