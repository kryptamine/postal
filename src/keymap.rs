use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Clone, Copy)]
pub enum Action {
    Move(Move),
    /// Half a page of the Preview.
    Scroll(Scroll),
    /// The selected Message in `$PAGER`.
    Open,
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

pub const fn action(key: KeyEvent) -> Option<Action> {
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
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
        KeyCode::Char('q') => Some(Action::Quit),
        _ => None,
    }
}
