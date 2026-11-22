use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Frame, layout::Rect};

use crate::{
    domain::shown::Shown,
    ui::exit::Exit,
    ui::frame::theme::Theme,
    ui::input::keymap,
    ui::input::step::Steer,
    ui::input::step::Step,
    ui::popup::folder_picker::{FolderPicker, Purpose},
    ui::popup::layer_act::Act,
    ui::popup::picker_popup::PickerPopup,
};

/// What a key does while the Folder Picker is open.
#[derive(Clone, Copy)]
enum Pick {
    Type(char),
    Erase,
    Down,
    Up,
    /// Shows the selected Folder, or moves Messages to it.
    Choose,
    Close,
    Quit,
}

/// The Folder Picker takes every key; every character is the Query's.
pub fn key(picker: &mut FolderPicker, key: KeyEvent) -> Act {
    match pick(key) {
        Some(Pick::Type(c)) => picker.push(c),
        Some(Pick::Erase) => picker.pop(),
        Some(Pick::Down) => picker.down(),
        Some(Pick::Up) => picker.up(),
        Some(Pick::Choose) => match (picker.purpose(), picker.chosen()) {
            (Purpose::MoveTo(_), Some(Shown::Folder(folder))) => {
                return Act::Close(Step::Steer(Steer::MoveTo(folder.clone())));
            }
            (_, Some(Shown::LocalDrafts)) => return Act::Close(Step::LocalDrafts),
            (_, Some(folder)) => return Act::Close(Step::Steer(Steer::Show(folder.clone()))),
            (_, None) => {}
        },
        Some(Pick::Close) => return Act::Close(Step::Stay),
        Some(Pick::Quit) => return Act::Step(Step::Exit(Exit::Quit)),
        None => {}
    }

    Act::Stay
}

pub fn draw(picker: &FolderPicker, frame: &mut Frame, theme: &Theme, body: Rect) {
    let popup = PickerPopup { theme, picker };
    frame.set_cursor_position(popup.cursor(body));
    frame.render_widget(popup, body);
}

const fn pick(key: KeyEvent) -> Option<Pick> {
    if !keymap::pressed(key) {
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
