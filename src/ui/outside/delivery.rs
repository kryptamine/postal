use std::{
    io::{self, Write},
    process::{Command, Stdio},
};

/// How a desktop notification reaches the user (ADR 0025).
#[derive(Clone, Copy)]
pub enum Delivery {
    /// The terminal's own `OSC 9`, which it shows as the desktop's.
    Osc,
    /// `osascript`, on macOS.
    Osascript,
    /// `notify-send`, elsewhere.
    NotifySend,
}

impl Delivery {
    /// The terminal's escape where the terminal is known to show it, else the OS.
    pub fn from_env() -> Self {
        let var = |name| std::env::var(name).unwrap_or_default();
        let term = var("TERM");
        // tmux and screen drop an OSC they are not told to pass on.
        let multiplexed = std::env::var_os("TMUX").is_some() || term.starts_with("screen");
        let program = var("TERM_PROGRAM");
        let known = matches!(program.as_str(), "iTerm.app" | "WezTerm" | "ghostty")
            || term == "xterm-kitty"
            || std::env::var_os("KITTY_WINDOW_ID").is_some();
        if known && !multiplexed {
            Self::Osc
        } else if cfg!(target_os = "macos") {
            Self::Osascript
        } else {
            Self::NotifySend
        }
    }

    /// Shows `title` over `body`. A failure is only logged: the mail is
    /// in the Message List all the same.
    pub fn send(self, title: &str, body: &str) {
        let sent = match self {
            Self::Osc => osc(title, body),
            // Passed as arguments, so neither needs quoting in AppleScript.
            Self::Osascript => spawn(Command::new("osascript").args([
                "-e",
                "on run argv",
                "-e",
                "display notification (item 2 of argv) with title (item 1 of argv)",
                "-e",
                "end run",
                title,
                body,
            ])),
            Self::NotifySend => {
                spawn(Command::new("notify-send").args(["--app-name=Postal", "--", title, body]))
            }
        };

        if let Err(error) = sent {
            tracing::warn!("notifying of new mail failed: {error}");
        }
    }
}

fn osc(title: &str, body: &str) -> io::Result<()> {
    // A control character would end the escape early, or start another.
    let text: String = format!("{title}: {body}")
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let mut out = io::stdout().lock();
    write!(out, "\x1b]9;{text}\x07")?;
    out.flush()
}

/// Starts `command` and reaps it on a thread of its own, so the event loop
/// never waits on it.
fn spawn(command: &mut Command) -> io::Result<()> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}
