use std::io::{self, Write};

use base64::{Engine, engine::general_purpose::STANDARD};

/// Puts `text` on the clipboard through the terminal with OSC 52, which
/// reaches the local one over SSH too. A terminal without it ignores it.
pub fn copy(text: &str) -> io::Result<()> {
    let mut out = io::stdout();
    write!(out, "\x1b]52;c;{}\x07", STANDARD.encode(text))?;
    out.flush()
}
