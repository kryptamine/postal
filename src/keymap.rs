use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Clone, Copy)]
pub enum Action {
    Move(Move),
    /// Half a page of the Preview.
    Scroll(Scroll),
    /// The selected Message in `$PAGER`.
    Open,
    Help,
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
pub enum Scroll {
    Down,
    Up,
}

/// One row of the `?` popup.
pub struct Binding {
    pub group: &'static str,
    pub keys: &'static str,
    pub aliases: &'static str,
    pub description: &'static str,
    pub hint: Option<Hint>,
}

pub struct Hint {
    pub keys: &'static str,
    pub description: &'static str,
}

/// The keymap as the hint line and the `?` popup show it, in their order.
/// `action` below reads the same keys; change the two together.
pub const BINDINGS: [Binding; 8] = [
    Binding {
        group: "Navigate",
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
        keys: "g / G",
        aliases: "Home / End",
        description: "first / last Message",
        hint: None,
    },
    Binding {
        group: "Message",
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
        keys: "Enter",
        aliases: "l",
        description: "open the Message in $PAGER",
        hint: Some(Hint {
            keys: "enter",
            description: "open",
        }),
    },
    // Listed ahead of hint mode, which binds it; until then `o` does nothing.
    Binding {
        group: "Links",
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
        keys: "?",
        aliases: "",
        description: "this help",
        hint: None,
    },
    Binding {
        group: "App",
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
        keys: "^z",
        aliases: "",
        description: "suspend to the shell",
        hint: None,
    },
];

/// The hint line's last entry, kept however narrow the terminal.
pub const HELP_HINT: Hint = Hint {
    keys: "?:",
    description: "help",
};

pub const fn action(key: KeyEvent) -> Option<Action> {
    if !pressed(key) {
        return None;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Some(Action::Quit),
            KeyCode::Char('d') => Some(Action::Scroll(Scroll::Down)),
            KeyCode::Char('u') => Some(Action::Scroll(Scroll::Up)),
            KeyCode::Char('z') => Some(Action::Suspend),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Move(Move::Down)),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Move(Move::Up)),
        KeyCode::Char('g') | KeyCode::Home => Some(Action::Move(Move::First)),
        KeyCode::Char('G') | KeyCode::End => Some(Action::Move(Move::Last)),
        KeyCode::PageDown => Some(Action::Scroll(Scroll::Down)),
        KeyCode::PageUp => Some(Action::Scroll(Scroll::Up)),
        KeyCode::Enter | KeyCode::Char('l') => Some(Action::Open),
        KeyCode::Char('?') => Some(Action::Help),
        KeyCode::Char('q') => Some(Action::Quit),
        _ => None,
    }
}

/// Whether `key` closes the `?` popup; while it is open, no other key acts.
pub const fn closes_help(key: KeyEvent) -> bool {
    pressed(key)
        && !key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('?' | 'q') | KeyCode::Esc)
}

const fn pressed(key: KeyEvent) -> bool {
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}
