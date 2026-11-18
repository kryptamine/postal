use std::{
    fmt,
    path::{Path, PathBuf},
};

use crate::credential::Credential;

const SERVICE: &str = "postal";

#[derive(Debug)]
pub enum Error {
    NoEntry {
        account: String,
    },
    Unavailable {
        source: keyring::Error,
        config: PathBuf,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoEntry { account } => write!(
                f,
                "the keychain has no entry for service `{SERVICE}`, account `{account}`"
            ),
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

/// Reads the credential of `account` from the OS keychain. It never falls back
/// to the file store; `config` is named in the hint to switch to it.
pub fn read(account: &str, config: &Path) -> Result<Credential, Error> {
    let secret = forced_failure()
        .map_or_else(
            || keyring::Entry::new(SERVICE, account)?.get_password(),
            Err,
        )
        .map_err(|source| match source {
            keyring::Error::NoEntry => Error::NoEntry {
                account: account.to_owned(),
            },
            source => Error::Unavailable {
                source,
                config: config.to_owned(),
            },
        })?;
    Ok(Credential::new(secret))
}

/// Test seam: a test cannot lock the real keychain, so `POSTAL_KEYCHAIN_FAIL`
/// makes it refuse access, with the variable's value as the reason.
fn forced_failure() -> Option<keyring::Error> {
    let reason = std::env::var("POSTAL_KEYCHAIN_FAIL").ok()?;
    Some(keyring::Error::NoStorageAccess(reason.into()))
}
