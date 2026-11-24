use std::{
    fs::{self, File, TryLockError},
    io,
    path::Path,
};

/// Held by the one process that writes the Cache and sends the Outbox, the
/// TUI or a folder sync, so no Message goes out twice. The OS lets it go
/// when the process ends, however it ends.
pub struct SyncLock {
    _held: File,
}

impl SyncLock {
    /// `None` while another process holds it.
    pub fn try_take(dir: &Path) -> io::Result<Option<Self>> {
        let file = open(dir)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _held: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }

    /// Waits while another process holds it.
    pub fn take(dir: &Path) -> io::Result<Self> {
        let file = open(dir)?;
        file.lock()?;
        Ok(Self { _held: file })
    }
}

fn open(dir: &Path) -> io::Result<File> {
    fs::create_dir_all(dir)?;
    File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("sync.lock"))
}
