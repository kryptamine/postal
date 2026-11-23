use std::io;

use ratatui::Terminal;

use crate::{
    render::rgb::{self, Rgb},
    ui::terminal::input::Input,
    ui::terminal::terminal_backend::{self, TerminalBackend},
    ui::terminal::terminal_background,
    ui::terminal::tty::Tty,
};

/// The terminal, taken onto the alternate screen only once something draws,
/// so an error found first reaches a plain terminal. Dropping hands it back.
pub struct TerminalSlot {
    taken: Option<(Terminal<TerminalBackend>, Rgb, Input)>,
}

impl TerminalSlot {
    pub const fn new() -> Self {
        Self { taken: None }
    }

    /// The terminal on the alternate screen, with its background colour and
    /// its input, which outlives a session: a key typed while one reloads
    /// reaches the next.
    pub fn take(&mut self) -> io::Result<(Tty<'_>, Rgb)> {
        let taken = if let Some(taken) = self.taken.take() {
            taken
        } else {
            let terminal = terminal_backend::init()?;
            // A terminal that does not say is taken for a dark one.
            let background = terminal_background::query().unwrap_or(rgb::BLACK);
            (terminal, background, Input::new())
        };

        let (terminal, background, input) = self.taken.insert(taken);
        Ok((Tty::new(terminal, input), *background))
    }
}

impl Drop for TerminalSlot {
    fn drop(&mut self) {
        if self.taken.is_some() {
            terminal_backend::restore();
        }
    }
}
