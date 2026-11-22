use std::{
    io,
    path::{Path, PathBuf},
};

use crate::{account::named_file, account::xdg};

/// Only the user may read their own mail.
const MODE: u32 = 0o600;

/// Where the attachments a Forward fetched are kept for the Draft at
/// `draft`: as long as it or its Message in the Outbox is, since a refusal
/// gives the Draft back with its `Attach:` lines.
fn dir(draft: &Path) -> io::Result<PathBuf> {
    let data = xdg::data_dir().map_err(io::Error::other)?;
    let stem = draft.file_stem().unwrap_or_default();
    Ok(data.join("forwarded").join(stem))
}

/// Writes the attachment at `index` of the Forward the Draft at `draft`
/// makes; returns where. Each has a directory of its own, so two of one
/// name both go out under it.
pub fn write(draft: &Path, index: usize, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    let dir = dir(draft)?.join(index.to_string());
    named_file::write_new(&dir, name, bytes, MODE)
}

/// Removes the attachments of each Forward whose Draft is gone from
/// `drafts/` and from `outbox/`, sent or discarded.
pub fn sweep() -> io::Result<()> {
    // Without a data directory no Draft, and so no Forward, was ever kept.
    let Ok(data) = xdg::data_dir() else {
        return Ok(());
    };

    let entries = match std::fs::read_dir(data.join("forwarded")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    for entry in entries {
        let entry = entry?;
        let stem = entry.file_name();
        let named = |dir: &str, extension: &str| {
            let mut name = stem.clone();
            name.push(extension);
            data.join(dir).join(name)
        };

        let kept = [("drafts", ".eml"), ("outbox", ".eml"), ("outbox", ".sent")]
            .into_iter()
            .any(|(dir, extension)| named(dir, extension).exists());
        if !kept && let Err(error) = std::fs::remove_dir_all(entry.path()) {
            tracing::warn!("removing {} failed: {error}", entry.path().display());
        }
    }

    Ok(())
}
