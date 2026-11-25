use std::{
    fs::{self, File, TryLockError},
    io,
    path::{Path, PathBuf},
};

use crate::account::xdg;

/// Held by the one process that writes the Cache and sends the Outbox, the
/// TUI or a folder sync, so no Message goes out twice. The OS lets it go
/// when the process ends, however it ends.
pub struct SyncLock {
    _held: [File; 2],
}

impl SyncLock {
    /// `None` while another process holds it.
    pub fn try_take() -> io::Result<Option<Self>> {
        let [lock, before] = files()?;
        if !try_lock(&lock)? || !try_lock(&before)? {
            return Ok(None);
        }

        Ok(Some(Self {
            _held: [lock, before],
        }))
    }

    /// Waits while another process holds it.
    pub fn take() -> io::Result<Self> {
        let [lock, before] = files()?;
        lock.lock()?;
        before.lock()?;
        Ok(Self {
            _held: [lock, before],
        })
    }
}

/// Where the lock lives: beside the Outbox it guards, so processes that
/// keep their Caches apart still send one Outbox once. Without a data
/// directory nothing can be sent, and the Cache's directory stands in.
pub fn dir() -> io::Result<PathBuf> {
    xdg::data_dir()
        .or_else(|_| xdg::cache_dir())
        .map_err(io::Error::other)
}

/// The lock, then the one 0.5.0 and older take in the Cache's directory,
/// held too until no such binary can still be running.
fn files() -> io::Result<[File; 2]> {
    let before = xdg::cache_dir().map_err(io::Error::other)?;
    Ok([
        open(&dir()?, "postal.lock")?,
        open(&before, "sync.lock")?,
    ])
}

fn try_lock(file: &File) -> io::Result<bool> {
    match file.try_lock() {
        Ok(()) => Ok(true),
        Err(TryLockError::WouldBlock) => Ok(false),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

fn open(dir: &Path, name: &str) -> io::Result<File> {
    fs::create_dir_all(dir)?;
    File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(name))
}
