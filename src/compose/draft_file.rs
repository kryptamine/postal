use std::{
    ffi::OsStr,
    fs::OpenOptions,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

use crate::{
    account::atomic_file, account::xdg, compose::draft_notes::DraftNotes, compose::notes::Notes,
    compose::unique_id::unique_id, domain::folder_name::FolderName, domain::uid::Uid,
};

/// The headers a Compose leaves to fill in, then the body.
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

/// What `$EDITOR` opens on Compose, with a line to write on above the
/// `signature`.
pub fn template(signature: Option<&str>) -> String {
    signature.map_or_else(
        || TEMPLATE.to_owned(),
        |signature| format!("{TEMPLATE}\n{signature}"),
    )
}

/// A new Draft's file in `drafts`, holding `text`.
pub fn create(drafts: &Path, text: &str) -> io::Result<PathBuf> {
    let path = named(drafts);
    create_at(&path, text)?;
    Ok(path)
}

/// Where a new Draft's file in `drafts` goes, before it is written.
pub fn named(drafts: &Path) -> PathBuf {
    drafts.join(format!("{}.eml", unique_id()))
}

/// The new Draft's file at `path`, holding `text`.
pub fn create_at(path: &Path, text: &str) -> io::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap_or(path))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(MODE)
        .open(path)?;
    file.write_all(text.as_bytes())
}

/// Whether the Draft at `path` is still the `template` it opened on, however
/// its blank lines and spaces were left. A file that does not read is not.
pub fn untouched(path: &Path, template: &str) -> bool {
    std::fs::read_to_string(path).is_ok_and(|text| {
        text.lines().all(|line| {
            let line = line.trim();
            line.is_empty() || template.lines().any(|written| written.trim() == line)
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
    folder: &FolderName,
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
        if theirs && original.is_some_and(|o| o.folder == *folder && o.uid == uid) {
            return Ok(Some(path));
        }
    }

    Ok(None)
}
