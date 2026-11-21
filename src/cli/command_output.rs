use std::io::{self, Write};

/// Writes a command's answer to stdout; a reader that stops early, such as
/// `head`, is no failure.
pub fn print(text: &str) -> io::Result<()> {
    match io::stdout().lock().write_all(text.as_bytes()) {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result,
    }
}
