use std::io;

use ratatui::{Terminal, layout::Rect};

use crate::{
    ui::frame::layout::{self, Panes},
    ui::run::Error,
    ui::state::mode::Mode,
    ui::terminal::input::Input,
    ui::terminal::terminal_backend::{self, TerminalBackend},
};

/// The terminal the TUI draws on, and the input read from it.
pub struct Tty<'a> {
    pub terminal: &'a mut Terminal<TerminalBackend>,
    pub events: &'a mut Input,
}

impl<'a> Tty<'a> {
    pub const fn new(terminal: &'a mut Terminal<TerminalBackend>, events: &'a mut Input) -> Self {
        Self { terminal, events }
    }

    /// Where the main screen's parts go in `mode` at the terminal's size.
    pub fn panes(&self, mode: Mode) -> Result<Panes, Error> {
        let size = self.terminal.size().map_err(Error::Terminal)?;
        Ok(layout::split(
            Rect::new(0, 0, size.width, size.height),
            mode,
        ))
    }

    /// Runs `program` in the foreground with the terminal handed back, then
    /// takes the terminal again. A failure of `program` alone is only logged.
    pub fn outside(&mut self, program: impl FnOnce() -> io::Result<()>) -> Result<(), Error> {
        // A live EventStream keeps reading the terminal, and would take the
        // pager's keys; dropping it stops its reader thread.
        self.events.restart();
        terminal_backend::leave(self.terminal).map_err(Error::Terminal)?;
        let result = program();
        terminal_backend::resume(self.terminal).map_err(Error::Terminal)?;
        if let Err(error) = result {
            tracing::error!("leaving the screen failed: {error}");
        }

        Ok(())
    }
}
