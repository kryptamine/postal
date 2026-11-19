use std::{
    fs::OpenOptions,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

use crate::{atomic_file, unique_id::unique_id};

/// What `$EDITOR` opens on Compose: the headers to fill in, then the body.
const TEMPLATE: &str = "To: \nCc: \nSubject: \n\n";

/// Only the user may read their own writing.
const MODE: u32 = 0o600;

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

/// Writes a discarded Draft's `text` back to its file.
pub fn restore(path: &Path, text: &str) -> io::Result<()> {
    atomic_file::write(path, text.as_bytes(), MODE)
}
