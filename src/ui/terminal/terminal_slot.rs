use std::io;

use ratatui::Terminal;

use crate::{
    render::rgb::{self, Rgb},
    ui::terminal::terminal_backend::{self, TerminalBackend},
    ui::terminal::terminal_background,
};

/// The terminal, taken onto the alternate screen only once something draws,
/// so an error found first reaches a plain terminal. Dropping hands it back.
pub struct TerminalSlot {
    taken: Option<(Terminal<TerminalBackend>, Rgb)>,
}

impl TerminalSlot {
    pub const fn new() -> Self {
        Self { taken: None }
    }

    /// The terminal on the alternate screen, with its background colour.
    pub fn take(&mut self) -> io::Result<(&mut Terminal<TerminalBackend>, Rgb)> {
        let taken = if let Some(taken) = self.taken.take() {
            taken
        } else {
            let terminal = terminal_backend::init()?;
            // A terminal that does not say is taken for a dark one.
            let background = terminal_background::query().unwrap_or(rgb::BLACK);
            (terminal, background)
        };

        let (terminal, background) = self.taken.insert(taken);
        Ok((terminal, *background))
    }
}

impl Drop for TerminalSlot {
    fn drop(&mut self) {
        if self.taken.is_some() {
            ratatui::restore();
        }
    }
}
