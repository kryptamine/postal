use std::io::{self, Write};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal,
};

/// Reads one line after `label`, starting from `initial` and echoing it only
/// when `echo` is set; `None` on Ctrl-C or Ctrl-D. Raw mode makes Ctrl-C a
/// key, so the terminal is restored before Postal exits.
pub fn read_line(label: &str, initial: &str, echo: bool) -> io::Result<Option<String>> {
    let mut out = io::stdout();
    write!(out, "{label} ")?;
    if echo {
        write!(out, "{initial}")?;
    }

    out.flush()?;
    terminal::enable_raw_mode()?;
    let line = edit(&mut out, initial.to_owned(), echo);
    terminal::disable_raw_mode()?;
    writeln!(out)?;
    line
}

fn edit(out: &mut impl Write, mut line: String, echo: bool) -> io::Result<Option<String>> {
    loop {
        let Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            ..
        }) = event::read()?
        else {
            continue;
        };

        let ctrl = modifiers.contains(KeyModifiers::CONTROL);
        match code {
            KeyCode::Char('c' | 'd') if ctrl => return Ok(None),
            KeyCode::Enter => return Ok(Some(line)),
            KeyCode::Backspace => {
                if line.pop().is_some() && echo {
                    write!(out, "\x08 \x08")?;
                }
            }
            KeyCode::Char(c) if !ctrl => {
                line.push(c);
                if echo {
                    write!(out, "{c}")?;
                }
            }
            _ => continue,
        }

        out.flush()?;
    }
}
