use std::{
    io,
    process::{Command, Stdio},
};

const OPENER: &str = if cfg!(target_os = "macos") {
    "open"
} else {
    "xdg-open"
};

/// Hands `url` to the desktop's opener and returns at once; a thread waits
/// for it. Its output would draw over the screen, so it has none.
pub fn open(url: &str) -> io::Result<()> {
    let mut child = Command::new(OPENER)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    std::thread::spawn(move || match child.wait() {
        Ok(status) if !status.success() => tracing::warn!("`{OPENER}` exited with {status}"),
        Ok(_) => {}
        Err(error) => tracing::warn!("waiting on `{OPENER}` failed: {error}"),
    });

    Ok(())
}
