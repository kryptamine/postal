use std::{fs, io, path::PathBuf};

use crate::{account::browser, account::named_file, account::xdg};

/// What a file opened or saved may be read by: the umask decides.
const MODE: u32 = 0o666;

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
    named_file::write_new(&dir, name, bytes, MODE)
}

/// Writes `bytes` as `name` and hands the file to the desktop's opener.
pub fn open(name: &str, bytes: &[u8]) -> io::Result<()> {
    let path = named_file::write_new(&opened_dir()?, name, bytes, MODE)?;
    browser::open(&path)
}
