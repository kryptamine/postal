use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// A key as the keymap binds it and the `?` popup names it.
#[derive(Clone, Copy)]
pub enum Key {
    /// One character, typed without Control; Shift is in the character.
    Char(&'static str),
    Ctrl(&'static str),
    Space,
    Enter,
    Esc,
    Tab,
    BackTab,
    Backspace,
    Up,
    Down,
    Left,
    Home,
    End,
    PageUp,
    PageDown,
}

impl Key {
    pub fn matches(self, key: KeyEvent) -> bool {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match (self, key.code) {
            (Self::Ctrl(name), KeyCode::Char(c)) => control && is(c, name),
            (Self::Char(name), KeyCode::Char(c)) => !control && is(c, name),
            _ if control => false,
            (Self::Space, KeyCode::Char(' '))
            | (Self::Enter, KeyCode::Enter)
            | (Self::Esc, KeyCode::Esc)
            | (Self::Tab, KeyCode::Tab)
            | (Self::BackTab, KeyCode::BackTab)
            | (Self::Backspace, KeyCode::Backspace)
            | (Self::Up, KeyCode::Up)
            | (Self::Down, KeyCode::Down)
            | (Self::Left, KeyCode::Left)
            | (Self::Home, KeyCode::Home)
            | (Self::End, KeyCode::End)
            | (Self::PageUp, KeyCode::PageUp)
            | (Self::PageDown, KeyCode::PageDown) => true,
            _ => false,
        }
    }

    /// Calls `part` with each piece of the names of `keys`, ` / ` between them.
    pub fn spell(keys: &[Self], mut part: impl FnMut(&'static str)) {
        for (i, key) in keys.iter().enumerate() {
            if i > 0 {
                part(" / ");
            }

            match *key {
                Self::Ctrl(name) => {
                    part("^");
                    part(name);
                }
                Self::Char(name) => part(name),
                Self::Space => part("Space"),
                Self::Enter => part("Enter"),
                Self::Esc => part("Esc"),
                Self::Tab => part("Tab"),
                Self::BackTab => part("S-Tab"),
                Self::Backspace => part("Backspace"),
                Self::Up => part("↑"),
                Self::Down => part("↓"),
                Self::Left => part("←"),
                Self::Home => part("Home"),
                Self::End => part("End"),
                Self::PageUp => part("PgUp"),
                Self::PageDown => part("PgDn"),
            }
        }
    }
}

fn is(c: char, name: &str) -> bool {
    c.encode_utf8(&mut [0; 4]) == name
}

pub const fn pressed(key: KeyEvent) -> bool {
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}
