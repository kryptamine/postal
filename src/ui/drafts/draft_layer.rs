use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Frame, layout::Rect};

use crate::{
    compose::accounts::Accounts,
    ui::drafts::draft_pane::DraftPane,
    ui::drafts::draft_preview::DraftPreview,
    ui::exit::Exit,
    ui::keymap::{self, Binding, Hint, Key},
    ui::mode::Mode,
    ui::popup::layer_act::Act,
    ui::step::{Drafting, Step},
    ui::theme::Theme,
};

/// What a key does while a Draft's preview is open.
#[derive(Clone, Copy)]
enum DraftKey {
    Send,
    Edit,
    /// Closes the preview, the Draft kept in Local Drafts.
    Keep,
    Discard,
    Help,
    Quit,
}

/// What a key does while an Outbox Message is shown read-only.
#[derive(Clone, Copy)]
enum QueuedKey {
    Close,
    Help,
    Quit,
}

const DRAFT: &[Mode] = &[Mode::Draft];

/// `draft` and `queued` below read the same keys; change them together.
pub const HELP: &[Binding] = &[
    Binding {
        group: "Draft",
        modes: DRAFT,
        signed_out: false,
        keys: &[Key::Char("y")],
        aliases: &[],
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
        keys: &[Key::Char("e")],
        aliases: &[],
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
        keys: &[Key::Char("q")],
        aliases: &[],
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
        keys: &[Key::Char("d")],
        aliases: &[],
        description: "keep it in Local Drafts",
        hint: Some(Hint {
            keys: "d",
            description: "keep",
        }),
    },
    Binding {
        group: "Outbox",
        modes: &[Mode::Queued],
        signed_out: false,
        keys: &[Key::Char("q")],
        aliases: &[Key::Esc],
        description: "back to Local Drafts",
        hint: Some(Hint {
            keys: "q",
            description: "back",
        }),
    },
];

/// A Draft's preview takes every key, and an Outbox Message's.
pub const fn key(preview: &DraftPreview, key: KeyEvent) -> Act {
    if preview.queued {
        return match queued(key) {
            Some(QueuedKey::Close) => Act::Close(Step::Stay),
            Some(QueuedKey::Help) => Act::Help,
            Some(QueuedKey::Quit) => Act::Step(Step::Exit(Exit::Quit)),
            None => Act::Stay,
        };
    }

    match draft(key) {
        Some(DraftKey::Quit) => Act::Step(Step::Exit(Exit::Quit)),
        Some(DraftKey::Help) => Act::Help,
        Some(DraftKey::Send) => Act::Step(Step::Draft(Drafting::Send)),
        Some(DraftKey::Edit) => Act::Step(Step::Draft(Drafting::Edit)),
        Some(DraftKey::Keep) => Act::Step(Step::Draft(Drafting::Keep)),
        Some(DraftKey::Discard) => Act::Step(Step::Draft(Drafting::Discard)),
        None => Act::Stay,
    }
}

pub const fn mode(preview: &DraftPreview) -> Mode {
    if preview.queued {
        Mode::Queued
    } else {
        Mode::Draft
    }
}

pub fn draw(
    preview: &DraftPreview,
    frame: &mut Frame,
    theme: &Theme,
    accounts: Accounts,
    body: Rect,
) {
    // The `From` a Send adds.
    let outgoing = accounts
        .place(preview.account.as_deref())
        .map(|account| accounts.outgoing(account));
    let from = outgoing.map(|outgoing| {
        let email = || outgoing.config.email.clone();
        outgoing
            .from()
            .map_or_else(|_bad| email(), |from| from.to_string())
    });

    let pane = DraftPane {
        theme,
        preview,
        from: from.as_deref().unwrap_or_default(),
    };

    frame.render_widget(pane, body);
}

const fn draft(key: KeyEvent) -> Option<DraftKey> {
    if !keymap::pressed(key) {
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

const fn queued(key: KeyEvent) -> Option<QueuedKey> {
    if !keymap::pressed(key) {
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
