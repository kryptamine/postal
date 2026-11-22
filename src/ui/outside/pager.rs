use std::{
    io::{self, Write},
    process::{Command, Stdio},
};

/// `-+F` undoes a `-F` in `$LESS`: quitting on text that fits one screen
/// would only flash the Message before the TUI takes the screen back.
const DEFAULT: &str = "less -R -+F";

/// Runs `$PAGER` through `sh`, so it may carry flags, with `text` on its
/// stdin, and waits for it to exit. No file is written.
pub fn page(text: &[u8]) -> io::Result<()> {
    let pager = std::env::var("PAGER")
        .ok()
        .filter(|pager| !pager.trim().is_empty())
        .unwrap_or_else(|| DEFAULT.to_owned());
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&pager)
        .stdin(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        // A pager quit before reading everything closes the pipe early.
        match stdin.write_all(text) {
            Err(error) if error.kind() != io::ErrorKind::BrokenPipe => {
                child.wait().ok();
                return Err(error);
            }
            _ => {}
        }
    }

    let status = child.wait()?;
    if !status.success() {
        tracing::warn!("pager `{pager}` exited with {status}");
    }

    Ok(())
}
