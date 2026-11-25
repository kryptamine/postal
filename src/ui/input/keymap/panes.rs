use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    compose::recipients::Recipients,
    ui::input::keymap::{Action, Binding, By, Direction, Hint, Key, Move, Scroll, pressed},
    ui::outside::open_outside::Outside,
    ui::outside::wanted_attachments::Purpose,
    ui::popup::layer,
    ui::state::mode::Mode,
};

/// A row of the panes' keymap and what its keys do.
struct PaneKeys {
    binding: Binding,
    /// What `binding.keys[i]` does, and `binding.aliases[i]` when there are
    /// aliases.
    actions: &'static [Action],
    /// Where the keys act; the Query's are listed with the list's.
    acts: &'static [Mode],
}

impl PaneKeys {
    fn action(&self, key: KeyEvent) -> Option<Action> {
        let at = |keys: &[Key]| keys.iter().position(|k| k.matches(key));
        let i = at(self.binding.keys).or_else(|| at(self.binding.aliases))?;
        self.actions.get(i).copied()
    }
}

/// Keys that act where they are listed.
const fn listed(binding: Binding, actions: &'static [Action]) -> PaneKeys {
    let acts = binding.modes;
    PaneKeys {
        binding,
        actions,
        acts,
    }
}

const LIST: &[Mode] = &[Mode::List];
const READING: &[Mode] = &[Mode::Reading];
const QUERY: &[Mode] = &[Mode::Query];
const TYPING: &[Mode] = &[Mode::Query, Mode::Menu];
const MENU: &[Mode] = &[Mode::Menu];
const BOTH: &[Mode] = &[Mode::List, Mode::Reading];
const ANY: &[Mode] = &[
    Mode::List,
    Mode::Reading,
    Mode::Draft,
    Mode::LocalDrafts,
    Mode::Queued,
    Mode::Settings,
];

const fn scroll(direction: Direction, by: By) -> Action {
    Action::Scroll(Scroll { direction, by })
}

const DOWN_UP: &[Key] = &[Key::Down, Key::Up];
const HOME_END: &[Key] = &[Key::Home, Key::End];
const PAGE: &[Key] = &[Key::Ctrl("d"), Key::Ctrl("u")];
const PAGE_KEYS: &[Key] = &[Key::PageDown, Key::PageUp];
const HALF_PAGE: &[Action] = &[
    scroll(Direction::Down, By::HalfPage),
    scroll(Direction::Up, By::HalfPage),
];
const MOVE: &[Action] = &[Action::Move(Move::Down), Action::Move(Move::Up)];

/// The panes' keymap and the keys every layer shares, as the hint line and
/// the `?` popup show them and `action` reads them.
const PANES: [PaneKeys; 42] = [
    listed(
        Binding {
            group: "Navigate",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Char("j"), Key::Char("k")],
            aliases: DOWN_UP,
            description: "next / previous Message",
            hint: Some(Hint {
                keys: "j/k",
                description: "move",
            }),
        },
        MOVE,
    ),
    listed(
        Binding {
            group: "Navigate",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Char("g"), Key::Char("G")],
            aliases: HOME_END,
            description: "first / last Message",
            hint: None,
        },
        &[Action::Move(Move::First), Action::Move(Move::Last)],
    ),
    listed(
        Binding {
            group: "Read",
            modes: READING,
            signed_out: false,
            keys: &[Key::Char("j"), Key::Char("k")],
            aliases: DOWN_UP,
            description: "scroll a line",
            hint: Some(Hint {
                keys: "j/k",
                description: "scroll",
            }),
        },
        &[
            scroll(Direction::Down, By::Line),
            scroll(Direction::Up, By::Line),
        ],
    ),
    listed(
        Binding {
            group: "Read",
            modes: READING,
            signed_out: false,
            keys: &[Key::Char("g"), Key::Char("G")],
            aliases: HOME_END,
            description: "top / bottom of the Thread",
            hint: None,
        },
        &[
            scroll(Direction::Up, By::End),
            scroll(Direction::Down, By::End),
        ],
    ),
    listed(
        Binding {
            group: "Read",
            modes: READING,
            signed_out: false,
            keys: PAGE,
            aliases: PAGE_KEYS,
            description: "scroll half a page",
            hint: Some(Hint {
                keys: "^d/^u",
                description: "page",
            }),
        },
        HALF_PAGE,
    ),
    listed(
        Binding {
            group: "Read",
            modes: READING,
            signed_out: false,
            keys: &[Key::Char("n"), Key::Char("p")],
            aliases: &[],
            description: "next / previous Message",
            hint: Some(Hint {
                keys: "n/p",
                description: "next",
            }),
        },
        &[Action::Step(Direction::Down), Action::Step(Direction::Up)],
    ),
    listed(
        Binding {
            group: "Read",
            modes: READING,
            signed_out: false,
            keys: &[Key::Enter],
            aliases: &[Key::Space],
            description: "fold / unfold the Message",
            hint: Some(Hint {
                keys: "enter",
                description: "fold",
            }),
        },
        &[Action::Fold],
    ),
    listed(
        Binding {
            group: "Read",
            modes: READING,
            signed_out: false,
            keys: &[Key::Char("|")],
            aliases: &[],
            description: "open the Message in $PAGER",
            hint: Some(Hint {
                keys: "|",
                description: "pager",
            }),
        },
        &[Action::Open(Outside::Pager)],
    ),
    listed(
        Binding {
            group: "Read",
            modes: READING,
            signed_out: false,
            keys: &[Key::Char("q"), Key::Esc],
            aliases: &[Key::Char("h"), Key::Left],
            description: "back to the list",
            hint: Some(Hint {
                keys: "q",
                description: "back",
            }),
        },
        &[Action::Back, Action::Back],
    ),
    PaneKeys {
        binding: Binding {
            group: "Message",
            modes: LIST,
            signed_out: false,
            keys: PAGE,
            aliases: PAGE_KEYS,
            description: "scroll the Preview half a page",
            hint: Some(Hint {
                keys: "^d/^u",
                description: "scroll",
            }),
        },
        actions: HALF_PAGE,
        acts: &[Mode::List, Mode::Query, Mode::Menu],
    },
    listed(
        Binding {
            group: "Message",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Enter],
            aliases: &[Key::Char("l")],
            description: "read the Message full screen",
            hint: Some(Hint {
                keys: "enter",
                description: "read",
            }),
        },
        &[Action::Read],
    ),
    listed(
        Binding {
            group: "Message",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Char("|")],
            aliases: &[],
            description: "open the Message in $PAGER",
            hint: None,
        },
        &[Action::Open(Outside::Pager)],
    ),
    listed(
        Binding {
            group: "Message",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Char("E")],
            aliases: &[],
            description: "open the Message in $EDITOR",
            hint: None,
        },
        &[Action::Open(Outside::Editor)],
    ),
    listed(
        Binding {
            group: "Message",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Char("J"), Key::Char("K")],
            aliases: &[],
            description: "pick Threads down / up",
            hint: None,
        },
        &[Action::Stretch(Move::Down), Action::Stretch(Move::Up)],
    ),
    listed(
        Binding {
            group: "Message",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Char("U")],
            aliases: &[],
            description: "mark Unread, or toggle the Picks",
            hint: None,
        },
        &[Action::MarkUnread],
    ),
    listed(
        Binding {
            group: "Message",
            modes: READING,
            signed_out: false,
            keys: &[Key::Char("U")],
            aliases: &[],
            description: "mark the Message Unread",
            hint: None,
        },
        &[Action::MarkUnread],
    ),
    listed(
        Binding {
            group: "Message",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Backspace],
            aliases: &[],
            description: "delete: move to Trash",
            hint: None,
        },
        &[Action::Delete],
    ),
    listed(
        Binding {
            group: "Message",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Char("e")],
            aliases: &[],
            description: "archive",
            hint: None,
        },
        &[Action::Archive],
    ),
    listed(
        Binding {
            group: "Message",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Char("m")],
            aliases: &[],
            description: "move to a Folder",
            hint: None,
        },
        &[Action::MoveTo],
    ),
    listed(
        Binding {
            group: "Message",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Char("u")],
            aliases: &[],
            description: "undo a move, discard or Send",
            hint: None,
        },
        &[Action::Undo],
    ),
    listed(
        Binding {
            group: "Message",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Char("c")],
            aliases: &[],
            description: "compose a new Message",
            hint: None,
        },
        &[Action::Compose],
    ),
    listed(
        Binding {
            group: "Message",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Char("r")],
            aliases: &[],
            description: "reply to the sender",
            hint: None,
        },
        &[Action::Reply(Recipients::Sender)],
    ),
    listed(
        Binding {
            group: "Message",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Char("R")],
            aliases: &[],
            description: "reply to all",
            hint: None,
        },
        &[Action::Reply(Recipients::All)],
    ),
    listed(
        Binding {
            group: "Message",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Char("F")],
            aliases: &[],
            description: "forward",
            hint: Some(Hint {
                keys: "F",
                description: "forward",
            }),
        },
        &[Action::Forward],
    ),
    listed(
        Binding {
            group: "Message",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Char("a"), Key::Char("s")],
            aliases: &[],
            description: "open / save an attachment",
            hint: None,
        },
        &[
            Action::Attachments(Purpose::Open),
            Action::Attachments(Purpose::Save),
        ],
    ),
    listed(
        Binding {
            group: "Links",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Char("o")],
            aliases: &[],
            description: "label the links in the Preview",
            hint: Some(Hint {
                keys: "o",
                description: "links",
            }),
        },
        &[Action::Links],
    ),
    listed(
        Binding {
            group: "View",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Char("v")],
            aliases: &[],
            description: "rendered / text view of HTML",
            hint: None,
        },
        &[Action::SwitchView],
    ),
    listed(
        Binding {
            group: "View",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Char("z")],
            aliases: &[],
            description: "show / hide the Quote",
            hint: None,
        },
        &[Action::SwitchQuoted],
    ),
    listed(
        Binding {
            group: "App",
            modes: BOTH,
            signed_out: true,
            keys: &[Key::Char("L")],
            aliases: &[],
            description: "sign in again",
            hint: Some(Hint {
                keys: "L",
                description: "sign in",
            }),
        },
        &[Action::SignIn],
    ),
    listed(
        Binding {
            group: "App",
            modes: BOTH,
            signed_out: false,
            keys: &[Key::Char(",")],
            aliases: &[],
            description: "settings and Accounts",
            hint: None,
        },
        &[Action::Settings],
    ),
    // The layers read `?` and `^c` themselves; these rows list them there.
    listed(
        Binding {
            group: "App",
            modes: ANY,
            signed_out: false,
            keys: &[Key::Char("?")],
            aliases: &[],
            description: "this help",
            hint: None,
        },
        &[Action::Help],
    ),
    listed(
        Binding {
            group: "App",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Char("q")],
            aliases: &[Key::Ctrl("c")],
            description: "quit",
            hint: Some(Hint {
                keys: "q",
                description: "quit",
            }),
        },
        &[Action::Quit],
    ),
    PaneKeys {
        binding: Binding {
            group: "App",
            modes: &[
                Mode::Reading,
                Mode::Draft,
                Mode::LocalDrafts,
                Mode::Queued,
                Mode::Settings,
            ],
            signed_out: false,
            keys: &[Key::Ctrl("c")],
            aliases: &[],
            description: "quit",
            hint: None,
        },
        actions: &[Action::Quit],
        acts: &[Mode::Reading, Mode::Query, Mode::Menu],
    },
    listed(
        Binding {
            group: "App",
            modes: READING,
            signed_out: false,
            keys: &[Key::Char("f")],
            aliases: &[],
            description: "pick the Folder to show",
            hint: None,
        },
        &[Action::Folders],
    ),
    listed(
        Binding {
            group: "Search",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Char("/")],
            aliases: &[],
            description: "search, or edit the Query",
            hint: Some(Hint {
                keys: "/",
                description: "search",
            }),
        },
        &[Action::Search],
    ),
    PaneKeys {
        binding: Binding {
            group: "Search",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Ctrl("j"), Key::Ctrl("k")],
            aliases: DOWN_UP,
            description: "move, while typing the Query",
            hint: None,
        },
        actions: MOVE,
        acts: QUERY,
    },
    PaneKeys {
        binding: Binding {
            group: "Search",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Tab, Key::BackTab],
            aliases: &[],
            description: "suggestions: next / previous",
            hint: None,
        },
        actions: &[
            Action::Complete(Direction::Down),
            Action::Complete(Direction::Up),
        ],
        acts: TYPING,
    },
    PaneKeys {
        binding: Binding {
            group: "Search",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Char("j"), Key::Char("k")],
            aliases: DOWN_UP,
            description: "in the menu: next / previous",
            hint: None,
        },
        actions: &[
            Action::Complete(Direction::Down),
            Action::Complete(Direction::Up),
        ],
        acts: MENU,
    },
    PaneKeys {
        binding: Binding {
            group: "Search",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Enter],
            aliases: &[],
            description: "in the menu: take the suggestion",
            hint: None,
        },
        actions: &[Action::Accept],
        acts: MENU,
    },
    PaneKeys {
        binding: Binding {
            group: "Search",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Enter],
            aliases: &[],
            description: "keep the Query",
            hint: None,
        },
        actions: &[Action::KeepQuery],
        acts: QUERY,
    },
    PaneKeys {
        binding: Binding {
            group: "Search",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Esc],
            aliases: &[],
            description: "drop the Picks, menu or Search",
            hint: None,
        },
        actions: &[Action::EndSearch],
        acts: &[Mode::List, Mode::Query, Mode::Menu],
    },
    listed(
        Binding {
            group: "Folders",
            modes: LIST,
            signed_out: false,
            keys: &[Key::Char("f")],
            aliases: &[],
            description: "pick the Folder to show",
            hint: Some(Hint {
                keys: "f",
                description: "folders",
            }),
        },
        &[Action::Folders],
    ),
];

// A key or an alias without its action would do nothing, silently.
const _: () = {
    let mut i = 0;
    while i < PANES.len() {
        let (binding, actions) = (&PANES[i].binding, PANES[i].actions.len());
        assert!(binding.keys.len() == actions);
        assert!(binding.aliases.is_empty() || binding.aliases.len() == actions);
        i += 1;
    }
};

/// The bindings listed in `mode` now: the panes' and the shared ones in
/// `PANES` order, then the layers'.
pub fn bindings(mode: Mode, signed_out: bool) -> impl Iterator<Item = &'static Binding> + Clone {
    PANES
        .iter()
        .map(|keys| &keys.binding)
        .chain(layer::help())
        .filter(move |b| b.modes.contains(&mode) && (signed_out || !b.signed_out))
}

/// The first row acting in `mode` that binds `key`; while the Query is
/// typed, any other character goes into it.
pub fn action(key: KeyEvent, mode: Mode) -> Option<Action> {
    if !pressed(key) {
        return None;
    }

    let bound = PANES
        .iter()
        .filter(|keys| keys.acts.contains(&mode))
        .find_map(|keys| keys.action(key));
    if bound.is_some() || !mode.typing() {
        return bound;
    }

    match key.code {
        _ if key.modifiers.contains(KeyModifiers::CONTROL) => None,
        KeyCode::Backspace => Some(Action::Erase),
        KeyCode::Char(c) => Some(Action::Type(c)),
        _ => None,
    }
}
