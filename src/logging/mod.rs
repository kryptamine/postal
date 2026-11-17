//! The log file: `postal.log` in the state directory, one run per file.

mod capped_file;

use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

use self::capped_file::CappedFile;

const CAP: u64 = 10 * 1024 * 1024;

/// Flushing drops the guard, which waits for the writer thread to drain.
static GUARD: Mutex<Option<WorkerGuard>> = Mutex::new(None);

#[derive(Debug)]
pub enum Error {
    Open { path: PathBuf, source: io::Error },
    Filter(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Filter(message) => write!(f, "POSTAL_LOG: {message}"),
        }
    }
}

impl std::error::Error for Error {}

pub fn path(state_dir: &Path) -> PathBuf {
    state_dir.join("postal.log")
}

/// Moves the previous run's log to `postal.log.1` and starts a new one.
/// `POSTAL_LOG` overrides `verbosity`; `RUST_LOG` is never read.
pub fn init(path: &Path, verbosity: usize) -> Result<(), Error> {
    let filter = match std::env::var("POSTAL_LOG") {
        Ok(directives) => {
            EnvFilter::try_new(directives).map_err(|e| Error::Filter(e.to_string()))?
        }
        Err(_) => EnvFilter::new(match verbosity {
            0 => "info",
            1 => "debug",
            _ => "trace",
        }),
    };

    let open_error = |source| Error::Open {
        path: path.to_owned(),
        source,
    };

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(open_error)?;
    }

    match fs::rename(path, previous(path)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(open_error(e)),
        _ => {}
    }

    let file = fs::File::create(path).map_err(open_error)?;
    let (writer, guard) = tracing_appender::non_blocking(CappedFile::new(file, cap()));
    // Without the `tracing-log` bridge, `log` records are dropped: async-imap's
    // trace logs the LOGIN command with the password in clear.
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .init();
    *GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(guard);
    Ok(())
}

/// Writes out every event logged so far; later events are lost.
pub fn flush() {
    let guard = GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    drop(guard);
}

fn previous(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".1");
    PathBuf::from(name)
}

/// Tests set `POSTAL_LOG_CAP` in bytes: 10 MB is too slow to fill through a pty.
fn cap() -> u64 {
    std::env::var("POSTAL_LOG_CAP")
        .ok()
        .and_then(|bytes| bytes.parse().ok())
        .unwrap_or(CAP)
}
