use std::{
    fmt,
    path::{Path, PathBuf},
};

use crate::credential::Credential;

const SERVICE: &str = "postal";

#[derive(Debug)]
pub enum Error {
    Unavailable {
        source: keyring::Error,
        config: PathBuf,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { source, config } => write!(
                f,
                "keychain unavailable or locked: {source}\n\
                 To keep the secret in a file instead, set `store = \"file\"` \
                 under `[secrets]` in {}",
                config.display()
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Reads the credential of `account` from the OS keychain, `None` without an
/// entry. It never falls back to the file store; `config` is named in the
/// hint to switch to it.
pub fn read(account: &str, config: &Path) -> Result<Option<Credential>, Error> {
    let secret = forced_failure().map_or_else(
        || keyring::Entry::new(SERVICE, account)?.get_password(),
        Err,
    );
    match secret {
        Ok(secret) => Ok(Some(Credential::new(secret))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(source) => Err(Error::Unavailable {
            source,
            config: config.to_owned(),
        }),
    }
}

/// Stores the credential of `account` in the OS keychain, replacing any.
pub fn write(account: &str, secret: &Credential, config: &Path) -> Result<(), Error> {
    forced_failure()
        .map_or_else(
            || keyring::Entry::new(SERVICE, account)?.set_password(secret.expose()),
            Err,
        )
        .map_err(|source| Error::Unavailable {
            source,
            config: config.to_owned(),
        })
}

/// Test seam: a test cannot lock the real keychain, so `POSTAL_KEYCHAIN_FAIL`
/// makes it refuse access, with the variable's value as the reason.
fn forced_failure() -> Option<keyring::Error> {
    let reason = std::env::var("POSTAL_KEYCHAIN_FAIL").ok()?;
    Some(keyring::Error::NoStorageAccess(reason.into()))
}
