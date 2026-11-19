use std::{
    ffi::OsStr,
    fs::OpenOptions,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

use crate::{answering::Answering, atomic_file, notes::Notes, unique_id::unique_id, xdg};

/// What `$EDITOR` opens on Compose: the headers to fill in, then the body.
const TEMPLATE: &str = "To: \nCc: \nSubject: \n\n";

/// Only the user may read their own writing.
const MODE: u32 = 0o600;

pub fn dir() -> io::Result<PathBuf> {
    let data = xdg::data_dir().map_err(io::Error::other)?;
    Ok(data.join("drafts"))
}

/// Every Draft's file in `drafts`, their notes left out.
pub fn listed(drafts: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(drafts) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };

    let mut paths = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension().and_then(OsStr::to_str) == Some("eml") {
            paths.push(path);
        }
    }

    Ok(paths)
}

/// A new Draft's file in `drafts`, holding the template, or `text` when given.
pub fn create(drafts: &Path, text: Option<&str>) -> io::Result<PathBuf> {
    std::fs::create_dir_all(drafts)?;
    let path = drafts.join(format!("{}.eml", unique_id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(MODE)
        .open(&path)?;
    file.write_all(text.unwrap_or(TEMPLATE).as_bytes())?;
    Ok(path)
}

/// Writes a discarded Draft's `text` back to its file, and for a Reply,
/// what it answers.
pub fn restore(path: &Path, text: &str, answering: Option<&Answering>) -> io::Result<()> {
    if let Some(answering) = answering {
        note(path, answering)?;
    }

    atomic_file::write(path, text.as_bytes(), MODE)
}

/// Notes beside the Draft at `path` that it is a Reply to `answering`.
pub fn note(path: &Path, answering: &Answering) -> io::Result<()> {
    let notes = Notes {
        answering: Some(answering.clone()),
        ..Notes::default()
    };

    notes.write(path, MODE)
}

/// The Draft in `drafts` answering the Message `uid` of `folder`, if any;
/// notes that do not read are skipped.
pub fn answering(drafts: &Path, (folder, uid): (&str, u32)) -> io::Result<Option<PathBuf>> {
    for path in listed(drafts)? {
        let notes = Notes::read(&path).unwrap_or_else(|error| {
            tracing::warn!("reading the notes of {} failed: {error}", path.display());
            None
        });
        let original = notes
            .and_then(|notes| notes.answering)
            .and_then(|answering| answering.original);
        if original.is_some_and(|(f, u)| *f == *folder && u == uid) {
            return Ok(Some(path));
        }
    }

    Ok(None)
}
