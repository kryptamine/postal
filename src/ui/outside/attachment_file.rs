use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use crate::{account::browser, account::xdg};

/// Where an attachment opened is written, emptied as the TUI starts.
fn opened_dir() -> io::Result<PathBuf> {
    Ok(xdg::cache_dir()
        .map_err(io::Error::other)?
        .join("attachments"))
}

/// Empties the attachments opened before: the programs showing them have
/// had them since.
pub fn clear() {
    let Ok(dir) = opened_dir() else {
        return;
    };

    match fs::remove_dir_all(&dir) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            tracing::warn!("emptying {} failed: {error}", dir.display());
        }
        _ => {}
    }
}

/// Writes `bytes` as `name` to the download directory, beside any file
/// of that name; returns where.
pub fn save(name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    let dir = xdg::download_dir().map_err(io::Error::other)?;
    write_new(&dir, name, bytes)
}

/// Writes `bytes` as `name` and hands the file to the desktop's opener.
pub fn open(name: &str, bytes: &[u8]) -> io::Result<()> {
    let path = write_new(&opened_dir()?, name, bytes)?;
    browser::open(&path)
}

/// A file of its own in `dir`: a taken name becomes `name (1).pdf`, and
/// an existing file is never written over.
fn write_new(dir: &Path, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let name = file_name(name);
    let mut taken = 0;
    loop {
        let path = dir.join(numbered(&name, taken));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(bytes)?;
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => taken += 1,
            Err(error) => return Err(error),
        }
    }
}

/// The name mail gave, kept inside the directory: a path separator
/// becomes `_`, and leading dots go, so it is neither hidden nor `..`.
fn file_name(sent: &str) -> String {
    let name: String = sent
        .trim()
        .trim_start_matches('.')
        .chars()
        .map(|c| if matches!(c, '/' | '\\') { '_' } else { c })
        .collect();
    if name.is_empty() {
        "attachment".to_owned()
    } else {
        name
    }
}

fn numbered(name: &str, taken: usize) -> String {
    if taken == 0 {
        return name.to_owned();
    }

    match name.rsplit_once('.').filter(|(stem, _)| !stem.is_empty()) {
        Some((stem, extension)) => format!("{stem} ({taken}).{extension}"),
        None => format!("{name} ({taken})"),
    }
}
