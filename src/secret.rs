use std::{
    fmt, io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use crate::{atomic_file, credential::Credential};

#[derive(Debug)]
pub enum Error {
    Read { path: PathBuf, source: io::Error },
    Write { path: PathBuf, source: io::Error },
    Exposed { path: PathBuf, mode: u32 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } | Self::Write { path, source } => {
                write!(f, "{}: {source}", path.display())
            }
            Self::Exposed { path, mode } => write!(
                f,
                "{}: mode is {mode:o}, other users must not read it; run `chmod 600` on it",
                path.display()
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Reads the credential from the file secret store, which must be `0600`.
pub fn read(path: &Path) -> Result<Credential, Error> {
    let read_error = |source| Error::Read {
        path: path.to_owned(),
        source,
    };

    let mode = std::fs::metadata(path)
        .map_err(read_error)?
        .permissions()
        .mode()
        & 0o777;
    if mode & 0o077 != 0 {
        return Err(Error::Exposed {
            path: path.to_owned(),
            mode,
        });
    }

    let secret = std::fs::read_to_string(path).map_err(read_error)?;
    Ok(Credential::new(secret.trim().to_owned()))
}

/// Stores the credential in the file secret store, readable by the owner only.
pub fn write(path: &Path, secret: &Credential) -> Result<(), Error> {
    atomic_file::write(path, format!("{}\n", secret.expose()).as_bytes(), 0o600).map_err(|source| {
        Error::Write {
            path: path.to_owned(),
            source,
        }
    })
}
