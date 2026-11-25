use std::{
    io,
    path::Path,
    process::{Command, ExitStatus},
};

use crate::{account::config::ComposeFormat, compose::editor_exit::EditorExit};

/// What a Draft's editor finds its Account's `compose-format` in.
const FORMAT_VAR: &str = "POSTAL_COMPOSE_FORMAT";
/// What a Draft's editor finds the status that sends it in.
const SEND_VAR: &str = "POSTAL_SEND_STATUS";
/// Not 1, which an editor's own error and a bare `:cquit` exit with (ADR 0037).
const SEND_STATUS: i32 = 100;

/// Runs `$VISUAL`, else `$EDITOR`, else `vi` through `sh`, so it may carry
/// flags, on `path`, and waits for it to exit.
pub fn edit(path: &Path) -> io::Result<()> {
    run(path, None, false).map(drop)
}

/// [`edit`] on a Draft that goes out in `format`.
pub fn edit_draft(path: &Path, format: ComposeFormat) -> io::Result<()> {
    run(path, Some(format), false).map(drop)
}

/// [`edit_draft`] that the editor may end by asking for the Draft's Send.
pub fn edit_to_send(path: &Path, format: ComposeFormat) -> io::Result<EditorExit> {
    let status = run(path, Some(format), true)?;
    Ok(if status.code() == Some(SEND_STATUS) {
        EditorExit::Send
    } else {
        EditorExit::Saved
    })
}

fn run(path: &Path, format: Option<ComposeFormat>, sendable: bool) -> io::Result<ExitStatus> {
    let editor = ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|editor| !editor.trim().is_empty())
        .unwrap_or_else(|| "vi".to_owned());
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("sh")
        .arg(path);
    if let Some(format) = format {
        command.env(FORMAT_VAR, format.key());
    }

    if sendable {
        command.env(SEND_VAR, SEND_STATUS.to_string());
    }

    let status = command.status()?;
    let sent = sendable && status.code() == Some(SEND_STATUS);
    if !status.success() && !sent {
        tracing::warn!("editor `{editor}` exited with {status}");
    }

    Ok(status)
}
