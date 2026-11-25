use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    ui::exit::Exit,
    ui::frame::theme::Theme,
    ui::input::keymap::{Binding, Hint, Key},
    ui::input::step::Step,
    ui::popup::layer_act::Act,
    ui::popup::settings::{self, Settings},
    ui::popup::settings_popup::SettingsPopup,
    ui::state::mode::Mode,
};

const SETTINGS: &[Mode] = &[Mode::Settings];

/// `Settings::key` reads the same keys; change the two together.
pub const HELP: &[Binding] = &[
    Binding {
        group: "Settings",
        modes: SETTINGS,
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
        group: "Settings",
        modes: SETTINGS,
        signed_out: false,
        keys: &[Key::Tab, Key::BackTab],
        aliases: &[Key::Char("l"), Key::Char("h")],
        description: "next / previous section",
        hint: Some(Hint {
            keys: "tab",
            description: "section",
        }),
    },
    Binding {
        group: "Settings",
        modes: SETTINGS,
        signed_out: false,
        keys: &[Key::Enter],
        aliases: &[],
        description: "apply the value, edit a Signature, the Name or a command, add an Account",
        hint: Some(Hint {
            keys: "Enter",
            description: "change",
        }),
    },
    Binding {
        group: "Settings",
        modes: SETTINGS,
        signed_out: false,
        keys: &[Key::Char("d")],
        aliases: &[Key::Backspace],
        description: "unlink the Account",
        // Only the Accounts tab unlinks: the popup's own footer says so there.
        hint: None,
    },
    Binding {
        group: "Settings",
        modes: SETTINGS,
        signed_out: false,
        keys: &[Key::Char("q")],
        aliases: &[Key::Esc, Key::Char(",")],
        description: "back to the Message List",
        hint: Some(Hint {
            keys: "q",
            description: "back",
        }),
    },
];

/// Settings takes every key.
pub fn key(settings: &mut Settings, key: KeyEvent) -> Act {
    match settings.key(key) {
        settings::Step::Stay => Act::Stay,
        settings::Step::Close { reload: true } => Act::Close(Step::Exit(Exit::Reload)),
        settings::Step::Close { reload: false } => Act::Close(Step::Stay),
        settings::Step::Help => Act::Help,
        settings::Step::Icons(choice) => Act::Step(Step::Icons(choice)),
        settings::Step::Notify(choice) => Act::Step(Step::Notify(choice)),
        settings::Step::Contacts(choice) => Act::Step(Step::Contacts(choice)),
        settings::Step::Signature(email) => Act::Step(Step::Signature(email)),
        settings::Step::AddAccount => Act::Step(Step::Exit(Exit::AddAccount)),
        settings::Step::Unlink(email) => Act::Step(Step::Exit(Exit::Unlink(email))),
        settings::Step::Quit => Act::Step(Step::Exit(Exit::Quit)),
    }
}

pub fn draw(settings: &Settings, frame: &mut Frame, theme: &Theme, body: Rect) {
    let popup = SettingsPopup { theme, settings };
    if let Some(cursor) = popup.cursor(body) {
        frame.set_cursor_position(cursor);
    }

    frame.render_widget(popup, body);
}
