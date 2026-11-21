use std::{
    io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use crate::{atomic_file, credential::Credential};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: {source}", .path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("{}: {source}", .path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error(
        "{}: mode is {mode:o}, other users must not read it; run `chmod 600` on it",
        .path.display()
    )]
    Exposed { path: PathBuf, mode: u32 },
}

/// Reads the credential from the file secret store, which must be `0600`;
/// `None` while there is no file.
pub fn read(path: &Path) -> Result<Option<Credential>, Error> {
    let read_error = |source| Error::Read {
        path: path.to_owned(),
        source,
    };

    let metadata = match std::fs::metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        metadata => metadata.map_err(read_error)?,
    };

    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(Error::Exposed {
            path: path.to_owned(),
            mode,
        });
    }

    let secret = std::fs::read_to_string(path).map_err(read_error)?;
    Ok(Some(Credential::new(secret.trim().to_owned())))
}

/// Where the file secret store under `dir` keeps the credential of `email`.
pub fn path(dir: &Path, email: &str) -> PathBuf {
    dir.join("secrets")
        .join(email.to_lowercase().replace('/', "%2F"))
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

/// Removes the credential from the file secret store; there being none is no error.
pub fn delete(path: &Path) -> Result<(), Error> {
    match std::fs::remove_file(path) {
        Err(source) if source.kind() != io::ErrorKind::NotFound => Err(Error::Write {
            path: path.to_owned(),
            source,
        }),
        _ => Ok(()),
    }
}
