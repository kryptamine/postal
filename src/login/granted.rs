use std::path::Path;

use super::Writes;
use crate::{
    atomic_file, cache,
    config::{Config, SecretStore},
    keychain, secret,
    sign_in::SignIn,
    xdg,
};

/// A sign-in the IMAP server accepted, not yet written.
pub struct Granted {
    pub(super) config: Config,
    pub(super) sign_in: SignIn,
    pub(super) writes: Writes,
}

impl Granted {
    /// The other Account the config names, which saving replaces.
    pub fn replaces(&self) -> Option<&str> {
        match &self.writes {
            Writes::Account(current) => Some(current),
            Writes::SecretOnly | Writes::Config => None,
        }
    }

    pub fn email(&self) -> &str {
        &self.config.email
    }

    /// Writes the secret, then the config when it changed.
    pub fn save(self, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = dir.join("config.toml");
        let config = &self.config;
        // The secret goes first, so a config never names an Account without one.
        match config.secrets {
            SecretStore::Keychain => {
                keychain::write(&config.email, self.sign_in.secret(), &config_path)?;
            }
            SecretStore::File => secret::write(&dir.join("secret"), self.sign_in.secret())?,
        }

        // Before the config, so the new Account never opens the old one's Cache.
        if matches!(self.writes, Writes::Account(_)) {
            cache::delete(&xdg::cache_dir()?)?;
        }

        if !matches!(self.writes, Writes::SecretOnly) {
            atomic_file::write(&config_path, config.to_toml().as_bytes(), 0o644)?;
        }

        Ok(())
    }
}
