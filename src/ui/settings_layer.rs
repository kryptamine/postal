use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    ui::exit::Exit,
    ui::keymap::{Binding, Hint},
    ui::layer_act::Act,
    ui::mode::Mode,
    ui::settings::{self, Settings},
    ui::settings_popup::SettingsPopup,
    ui::step::Step,
    ui::theme::Theme,
};

const SETTINGS: &[Mode] = &[Mode::Settings];

/// `Settings::key` reads the same keys; change the two together.
pub const HELP: &[Binding] = &[
    Binding {
        group: "Settings",
        modes: SETTINGS,
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
        group: "Settings",
        modes: SETTINGS,
        signed_out: false,
        keys: "Enter",
        aliases: "",
        description: "change the setting, or add an Account",
        hint: Some(Hint {
            keys: "Enter",
            description: "change",
        }),
    },
    Binding {
        group: "Settings",
        modes: SETTINGS,
        signed_out: false,
        keys: "d",
        aliases: "Backspace",
        description: "unlink the Account",
        hint: Some(Hint {
            keys: "d",
            description: "unlink",
        }),
    },
    Binding {
        group: "Settings",
        modes: SETTINGS,
        signed_out: false,
        keys: "q",
        aliases: "Esc / ,",
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
