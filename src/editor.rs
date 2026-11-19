use std::{io, path::Path, process::Command};

/// Runs `$VISUAL`, else `$EDITOR`, else `vi` through `sh`, so it may carry
/// flags, on `path`, and waits for it to exit.
pub fn edit(path: &Path) -> io::Result<()> {
    let editor = ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|editor| !editor.trim().is_empty())
        .unwrap_or_else(|| "vi".to_owned());
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("sh")
        .arg(path)
        .status()?;
    if !status.success() {
        tracing::warn!("editor `{editor}` exited with {status}");
    }

    Ok(())
}
