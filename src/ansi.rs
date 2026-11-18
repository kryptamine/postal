use std::io::{self, Write};

use ratatui::{
    backend::IntoCrossterm,
    crossterm::{
        queue,
        style::{SetBackgroundColor, SetForegroundColor},
    },
    style::{Modifier, Style},
};

/// SGR codes for the modifiers a cell can carry. Bold comes after dim:
/// some terminals keep one intensity, where the last wins, as in ratatui.
const MODIFIERS: [(Modifier, u8); 9] = [
    (Modifier::DIM, 2),
    (Modifier::BOLD, 1),
    (Modifier::ITALIC, 3),
    (Modifier::UNDERLINED, 4),
    (Modifier::SLOW_BLINK, 5),
    (Modifier::RAPID_BLINK, 6),
    (Modifier::REVERSED, 7),
    (Modifier::HIDDEN, 8),
    (Modifier::CROSSED_OUT, 9),
];

pub const RESET: &[u8] = b"\x1b[0m";

/// A full reset, then `modifier`: the colours are reset too.
pub fn modifiers(out: &mut impl Write, modifier: Modifier) -> io::Result<()> {
    out.write_all(RESET)?;
    added(out, modifier)
}

fn added(out: &mut impl Write, modifier: Modifier) -> io::Result<()> {
    for (flag, code) in MODIFIERS {
        if modifier.contains(flag) {
            write!(out, "\x1b[{code}m")?;
        }
    }

    Ok(())
}

/// `text` in `style` over plain text, with every attribute reset after it.
pub fn styled(out: &mut impl Write, text: &str, style: Style) -> io::Result<()> {
    if style == Style::new() {
        return out.write_all(text.as_bytes());
    }

    added(out, style.add_modifier)?;
    if let Some(fg) = style.fg {
        queue!(out, SetForegroundColor(fg.into_crossterm()))?;
    }

    if let Some(bg) = style.bg {
        queue!(out, SetBackgroundColor(bg.into_crossterm()))?;
    }

    out.write_all(text.as_bytes())?;
    out.write_all(RESET)
}
