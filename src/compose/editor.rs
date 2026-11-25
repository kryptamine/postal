use std::{io, path::Path, process::Command};

use crate::account::config::ComposeFormat;

/// What a Draft's editor finds its Account's `compose-format` in.
const FORMAT_VAR: &str = "POSTAL_COMPOSE_FORMAT";

/// Runs `$VISUAL`, else `$EDITOR`, else `vi` through `sh`, so it may carry
/// flags, on `path`, and waits for it to exit.
pub fn edit(path: &Path) -> io::Result<()> {
    run(path, None)
}

/// [`edit`] on a Draft that goes out in `format`.
pub fn edit_draft(path: &Path, format: ComposeFormat) -> io::Result<()> {
    run(path, Some(format))
}

fn run(path: &Path, format: Option<ComposeFormat>) -> io::Result<()> {
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

    let status = command.status()?;
    if !status.success() {
        tracing::warn!("editor `{editor}` exited with {status}");
    }

    Ok(())
}
