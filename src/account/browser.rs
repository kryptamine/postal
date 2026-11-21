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

/// Mail is untrusted: a `file:` URL or one read as an option could start a
/// program, and a control character could end an escape sequence early, so
/// only plain web and mail links leave Postal.
pub fn safe(url: &str) -> bool {
    let web = ["http://", "https://", "mailto:"].iter().any(|scheme| {
        url.get(..scheme.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(scheme))
    });
    web && !url.chars().any(char::is_control)
}
