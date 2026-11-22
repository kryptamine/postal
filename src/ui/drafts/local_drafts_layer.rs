use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Frame, layout::Rect};

use crate::{
    compose::accounts::Accounts,
    compose::local_drafts::LocalDrafts,
    ui::drafts::draft_preview::DraftPreview,
    ui::drafts::local_drafts_pane::LocalDraftsPane,
    ui::exit::Exit,
    ui::keymap::{self, Binding, Hint, Key},
    ui::mode::Mode,
    ui::popup::layer_act::Act,
    ui::step::{Drafting, Steer, Step},
    ui::theme::Theme,
    ui::toast::Toast,
};

/// What a key does in Local Drafts.
#[derive(Clone, Copy)]
enum LocalKey {
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

const LOCAL: &[Mode] = &[Mode::LocalDrafts];

/// `local` below reads the same keys; change the two together.
pub const HELP: &[Binding] = &[
    Binding {
        group: "Local Drafts",
        modes: LOCAL,
        signed_out: false,
        keys: &[Key::Char("j"), Key::Char("k")],
        aliases: &[Key::Down, Key::Up],
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
        keys: &[Key::Enter],
        aliases: &[],
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
        keys: &[Key::Backspace],
        aliases: &[],
        description: "discard the Draft",
        hint: None,
    },
    Binding {
        group: "Local Drafts",
        modes: LOCAL,
        signed_out: false,
        keys: &[Key::Char("u")],
        aliases: &[],
        description: "undo a discard",
        hint: None,
    },
    Binding {
        group: "Local Drafts",
        modes: LOCAL,
        signed_out: false,
        keys: &[Key::Char("q")],
        aliases: &[Key::Esc],
        description: "back to the Message List",
        hint: Some(Hint {
            keys: "q",
            description: "back",
        }),
    },
];

/// Local Drafts take every key.
pub fn key(local: &mut LocalDrafts, key: KeyEvent) -> Act {
    match self::local(key) {
        Some(LocalKey::Down) => local.down(),
        Some(LocalKey::Up) => local.up(),
        Some(LocalKey::Open) => {
            return local
                .chosen()
                .map_or(Act::Stay, |row| open(row.path.clone(), row.queued));
        }
        Some(LocalKey::Discard) => {
            if let Some(row) = local.chosen().filter(|row| !row.queued) {
                return Act::Step(Step::Draft(Drafting::DiscardFile(row.path.clone())));
            }
        }
        Some(LocalKey::Undo) => return Act::Step(Step::Steer(Steer::Undo)),
        Some(LocalKey::Close) => return Act::Close(Step::Stay),
        Some(LocalKey::Help) => return Act::Help,
        Some(LocalKey::Quit) => return Act::Step(Step::Exit(Exit::Quit)),
        None => {}
    }

    Act::Stay
}

pub fn draw(local: &LocalDrafts, frame: &mut Frame, theme: &Theme, accounts: Accounts, body: Rect) {
    let pane = LocalDraftsPane {
        theme,
        local,
        accounts,
    };

    frame.render_widget(pane, body);
}

/// The preview of the Draft, or the Outbox Message when `queued`, at `path`.
fn open(path: PathBuf, queued: bool) -> Act {
    let read = if queued {
        DraftPreview::queued(path)
    } else {
        DraftPreview::read(path, None)
    };

    match read {
        Ok(preview) => Act::Open(Box::new(preview.into())),
        Err(error) => {
            tracing::error!("reading the Draft failed: {error}");
            Act::Toast(Toast::error("could not read the Draft"))
        }
    }
}

const fn local(key: KeyEvent) -> Option<LocalKey> {
    if !keymap::pressed(key) {
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
