use termlens::{Key, Screen, Terminal};

use crate::support::picker::picked;

/// `$EDITOR` running `command` on the Draft, then exiting as `:PostalSend`
/// does; the `:` takes the path Postal appends.
pub fn sending(command: &str) -> String {
    format!("{command} \"$1\"; exit \"$POSTAL_SEND_STATUS\"; :")
}

pub fn saved(screen: &Screen) -> bool {
    screen.contains("Saved to Drafts") && !screen.contains("y send")
}

/// From the screen an editor that only saved left, the preview of the
/// newest Draft, opened from Local Drafts, where its Send returns to.
pub fn preview_newest(t: &mut Terminal) {
    t.wait_until(saved).unwrap();
    t.send(Key::Char('f')).unwrap();
    t.send_str("local").unwrap();
    t.wait_until(|s| picked(s).first().is_some_and(|f| f == "Local Drafts"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains(" Local Drafts ")).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains(" Draft ") && s.contains("y send"))
        .unwrap();
}
