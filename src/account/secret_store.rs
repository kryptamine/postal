use std::{error::Error, path::Path};

use crate::{
    account::credential::Credential, account::keychain, account::secret, account::written::written,
};

written! {
    /// Where every Account's secret is kept; `dir` is the config directory.
    #[derive(Default, Clone, Copy, PartialEq, Eq)]
    pub enum SecretStore {
        #[default]
        Keychain = "keychain",
        File = "file",
    }
}

impl SecretStore {
    /// The secret of `email`; `None` without one.
    pub fn read(self, dir: &Path, email: &str) -> Result<Option<Credential>, Box<dyn Error>> {
        Ok(match self {
            Self::Keychain => keychain::read(email, &dir.join("config.toml"))?,
            Self::File => secret::read(&secret::path(dir, email))?,
        })
    }

    /// Replaces any secret of `email`.
    pub fn write(self, dir: &Path, email: &str, secret: &Credential) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Keychain => keychain::write(email, secret, &dir.join("config.toml"))?,
            Self::File => secret::write(&secret::path(dir, email), secret)?,
        }

        Ok(())
    }

    pub fn delete(self, dir: &Path, email: &str) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Keychain => keychain::delete(email, &dir.join("config.toml"))?,
            Self::File => secret::delete(&secret::path(dir, email))?,
        }

        Ok(())
    }
}
