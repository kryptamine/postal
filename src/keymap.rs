use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Clone, Copy)]
pub enum Action {
    Move(Move),
    Quit,
}

#[derive(Clone, Copy)]
pub enum Move {
    Down,
    Up,
    First,
    Last,
}

pub const fn action(key: KeyEvent) -> Option<Action> {
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return None;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Some(Action::Quit),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Move(Move::Down)),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Move(Move::Up)),
        KeyCode::Char('g') | KeyCode::Home => Some(Action::Move(Move::First)),
        KeyCode::Char('G') | KeyCode::End => Some(Action::Move(Move::Last)),
        KeyCode::Char('q') => Some(Action::Quit),
        _ => None,
    }
}
