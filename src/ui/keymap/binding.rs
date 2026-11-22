use crate::{ui::keymap::Key, ui::mode::Mode};

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

pub struct Hint {
    pub keys: &'static str,
    pub description: &'static str,
}

/// The hint line's last entry, kept however narrow the terminal.
pub const HELP_HINT: Hint = Hint {
    keys: "?:",
    description: "help",
};
