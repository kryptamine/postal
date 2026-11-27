use std::{
    fs::{self, File, TryLockError},
    io,
    os::unix::fs::FileExt,
    path::{Path, PathBuf},
};

use crate::account::xdg;

/// Held by the one process that writes the Cache and sends the Outbox, the
/// TUI or a folder sync, so no Message goes out twice. The OS lets it go
/// when the process ends, however it ends. Its file names the holder.
pub struct SyncLock {
    lock: File,
    _before: File,
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Tui,
    /// A folder sync, perhaps inside a long-lived `postal mcp`.
    Sync,
}

/// What a holder is doing that a takeover waits on.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Busy {
    /// A program, the editor or the pager, has the terminal.
    pub outside: bool,
    /// SMTP has a Message that is not yet accepted.
    pub sending: bool,
}

/// The holder its lock file names.
#[derive(Debug)]
pub struct Holder {
    pub kind: Kind,
    pub pid: u32,
    /// `None` when it has no terminal.
    pub tty: Option<String>,
    pub busy: Busy,
}

impl SyncLock {
    /// `None` while another process holds it.
    pub fn try_take(kind: Kind) -> io::Result<Option<Self>> {
        let [lock, before] = files()?;
        if !try_lock(&lock)? || !try_lock(&before)? {
            return Ok(None);
        }

        Self::held(lock, before, kind).map(Some)
    }

    fn held(lock: File, before: File, kind: Kind) -> io::Result<Self> {
        let held = Self {
            lock,
            _before: before,
            kind,
        };

        held.note(Busy::default())?;
        Ok(held)
    }

    /// Writes into the lock file who holds it, doing `busy`.
    pub fn note(&self, busy: Busy) -> io::Result<()> {
        let kind = match self.kind {
            Kind::Tui => "tui",
            Kind::Sync => "sync",
        };
        let tty = tty().unwrap_or_else(|| "-".to_owned());
        let line = format!(
            "{kind} {} {tty} {} {}\n",
            std::process::id(),
            u8::from(busy.outside),
            u8::from(busy.sending)
        );

        self.lock.write_all_at(line.as_bytes(), 0)?;
        self.lock
            .set_len(u64::try_from(line.len()).unwrap_or(u64::MAX))
    }
}

/// Who holds the lock, as its file names it: `None` for a holder that
/// writes none, as 0.5.0 and older do, or one gone since.
pub fn holder() -> io::Result<Option<Holder>> {
    let text = match fs::read_to_string(dir()?.join(LOCK)) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut words = text.split_whitespace();
    let (Some(kind), Some(pid), Some(tty), Some(outside), Some(sending)) = (
        words.next(),
        words.next(),
        words.next(),
        words.next(),
        words.next(),
    ) else {
        return Ok(None);
    };

    let kind = match kind {
        "tui" => Kind::Tui,
        "sync" => Kind::Sync,
        _ => return Ok(None),
    };
    let Ok(pid) = pid.parse() else {
        return Ok(None);
    };

    Ok(Some(Holder {
        kind,
        pid,
        tty: (tty != "-").then(|| tty.to_owned()),
        busy: Busy {
            outside: outside == "1",
            sending: sending == "1",
        },
    }))
}

/// The terminal on standard input, as `/dev/ttys003`.
pub fn tty() -> Option<String> {
    let mut name = [0u8; 256];
    // SAFETY: the buffer is writable for the length given.
    let failed =
        unsafe { libc::ttyname_r(libc::STDIN_FILENO, name.as_mut_ptr().cast(), name.len()) };
    if failed != 0 {
        return None;
    }

    let end = name.iter().position(|&byte| byte == 0)?;
    String::from_utf8(name.get(..end)?.to_vec()).ok()
}

const LOCK: &str = "postal.lock";

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
    Ok([open(&dir()?, LOCK)?, open(&before, "sync.lock")?])
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
        .read(true)
        .write(true)
        .open(dir.join(name))
}
