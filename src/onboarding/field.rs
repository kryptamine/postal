use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// What a key does to a text field on the Onboarding.
pub enum Edit {
    Changed,
    Submit,
    Back,
    Cancel,
    Ignored,
}

/// Applies `event` to `value`: every printable key types, `q` and `j` too.
pub fn edit(value: &mut String, event: KeyEvent) -> Edit {
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    match event.code {
        KeyCode::Char('c') if ctrl => Edit::Cancel,
        KeyCode::Char('u') if ctrl => {
            value.clear();
            Edit::Changed
        }
        _ if ctrl => Edit::Ignored,
        KeyCode::Char(c) => {
            value.push(c);
            Edit::Changed
        }
        KeyCode::Backspace => {
            value.pop();
            Edit::Changed
        }
        KeyCode::Enter => Edit::Submit,
        KeyCode::Esc => Edit::Back,
        _ => Edit::Ignored,
    }
}
