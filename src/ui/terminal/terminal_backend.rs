use std::io::{self, Stdout, Write};

use ratatui::{
    Terminal,
    backend::{Backend, ClearType, CrosstermBackend, IntoCrossterm, WindowSize},
    buffer::Cell,
    crossterm::{
        cursor::MoveTo,
        execute, queue,
        style::{Color as CrosstermColor, Colors, Print, SetColors, SetUnderlineColor},
        terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Position, Size},
    style::{Color, Modifier},
};

use crate::ui::terminal::ansi;

/// The crossterm backend with a `draw` that does not allocate: crossterm's
/// `SetAttribute` builds a `String` per call. ADR 0001 forbids that per frame.
pub struct TerminalBackend {
    inner: CrosstermBackend<Stdout>,
    out: Stdout,
}

/// Raw mode on the alternate screen, as `ratatui::init` sets it up; the
/// panic hook and `ratatui::restore` undo it.
pub fn init() -> io::Result<Terminal<TerminalBackend>> {
    terminal::enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    Terminal::new(TerminalBackend {
        inner: CrosstermBackend::new(io::stdout()),
        out: io::stdout(),
    })
}

/// Hands the terminal back as Postal found it, for a program run in the
/// foreground; `resume` takes it again.
pub fn leave(terminal: &mut Terminal<TerminalBackend>) -> io::Result<()> {
    terminal.show_cursor()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    terminal::disable_raw_mode()
}

/// Raw mode on the alternate screen again, cleared so the next draw
/// repaints every cell.
pub fn resume(terminal: &mut Terminal<TerminalBackend>) -> io::Result<()> {
    terminal::enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    terminal.hide_cursor()?;
    // Not `Terminal::clear`: it asks the terminal where the cursor is.
    // Swapping twice blanks both buffers, so the next draw has no diff base.
    terminal.backend_mut().clear()?;
    terminal.swap_buffers();
    terminal.swap_buffers();
    Ok(())
}

impl Backend for TerminalBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut out = self.out.lock();
        let mut fg = Color::Reset;
        let mut bg = Color::Reset;
        let mut underline = Color::Reset;
        let mut modifier = Modifier::empty();
        let mut last: Option<(u16, u16)> = None;
        for (x, y, cell) in content {
            if !matches!(last, Some((lx, ly)) if x == lx + 1 && y == ly) {
                queue!(out, MoveTo(x, y))?;
            }

            last = Some((x, y));
            // A full reset then the new set is simpler than a diff, but it
            // also resets the colours, so they are set again below.
            if cell.modifier != modifier {
                ansi::modifiers(&mut out, cell.modifier)?;
                modifier = cell.modifier;
                (fg, bg, underline) = (Color::Reset, Color::Reset, Color::Reset);
            }

            if cell.fg != fg || cell.bg != bg {
                let colors = Colors::new(cell.fg.into_crossterm(), cell.bg.into_crossterm());
                queue!(out, SetColors(colors))?;
                (fg, bg) = (cell.fg, cell.bg);
            }

            if cell.underline_color != underline {
                queue!(
                    out,
                    SetUnderlineColor(cell.underline_color.into_crossterm())
                )?;
                underline = cell.underline_color;
            }

            queue!(out, Print(cell.symbol()))?;
        }

        let reset = Colors::new(CrosstermColor::Reset, CrosstermColor::Reset);
        queue!(
            out,
            SetColors(reset),
            SetUnderlineColor(CrosstermColor::Reset)
        )?;
        out.write_all(ansi::RESET)
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Size> {
        self.inner.size()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}
