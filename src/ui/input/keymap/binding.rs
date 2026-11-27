use crate::{ui::input::keymap::Key, ui::state::mode::Mode};

/// One row of the `?` popup.
pub struct Binding {
    pub group: &'static str,
    pub modes: &'static [Mode],
    /// Listed, and active, only while signed out.
    pub signed_out: bool,
    pub keys: &'static [Key],
    pub aliases: &'static [Key],
    pub description: &'static str,
    pub hint: Option<Hint>,
}

impl Binding {
    /// Whether the `?` popup and the hint line list it in `mode`.
    pub fn shown_in(&self, mode: Mode, signed_out: bool) -> bool {
        self.modes.contains(&mode) && (signed_out || !self.signed_out)
    }
}

pub struct Hint {
    pub keys: &'static str,
    pub description: &'static str,
}

/// The hint line's last entry, kept however narrow the terminal.
pub const HELP_HINT: Hint = Hint {
    keys: "?:",
    description: "help",
};
