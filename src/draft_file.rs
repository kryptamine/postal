use std::{
    ffi::OsStr,
    fs::OpenOptions,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

use crate::{
    atomic_file, draft_notes::DraftNotes, notes::Notes, uid::Uid, unique_id::unique_id, xdg,
};

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

/// Whether the Draft at `path` is still the template, however its blank
/// lines and spaces were left. A file that does not read is not.
pub fn untouched(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|text| {
        text.lines().all(|line| {
            let line = line.trim();
            line.is_empty() || TEMPLATE.lines().any(|header| header.trim() == line)
        })
    })
}

/// Writes a discarded Draft's `text` back to its file, with its notes.
pub fn restore(path: &Path, text: &str, notes: &DraftNotes) -> io::Result<()> {
    note(path, notes)?;
    atomic_file::write(path, text.as_bytes(), MODE)
}

/// Writes the notes of the Draft at `path`, unless it has none.
pub fn note(path: &Path, notes: &DraftNotes) -> io::Result<()> {
    if notes.account.is_none() && notes.answering.is_none() {
        return Ok(());
    }

    let notes = Notes {
        account: notes.account.clone(),
        answering: notes.answering.clone(),
        ..Notes::default()
    };

    notes.write(path, MODE)
}

/// The Draft in `drafts` answering the Message `uid` of `folder` of the
/// Account `account`, if any; notes that do not read are skipped.
pub fn answering(
    drafts: &Path,
    account: &str,
    folder: &str,
    uid: Uid,
) -> io::Result<Option<PathBuf>> {
    for path in listed(drafts)? {
        let notes = Notes::read(&path).unwrap_or_else(|error| {
            tracing::warn!("reading the notes of {} failed: {error}", path.display());
            None
        });
        let Some(notes) = notes else {
            continue;
        };

        let theirs = notes
            .account
            .as_deref()
            .is_none_or(|a| a.eq_ignore_ascii_case(account));
        let original = notes.answering.and_then(|answering| answering.original);
        if theirs && original.is_some_and(|o| *o.folder == *folder && o.uid == uid) {
            return Ok(Some(path));
        }
    }

    Ok(None)
}
