use std::path::Path;

use super::Writes;
use crate::{
    atomic_file, cache, config::Config, partial_config::PartialConfig, provider::Provider,
    sign_in::SignIn, xdg,
};

/// A sign-in the IMAP server accepted, not yet written.
pub struct Granted {
    pub(super) partial: PartialConfig,
    pub(super) config: Config,
    pub(super) sign_in: SignIn,
    pub(super) writes: Writes,
}

impl Granted {
    /// The other Provider the config names this address at, which saving
    /// replaces.
    pub const fn replaces(&self) -> Option<Provider> {
        match self.writes {
            Writes::Moved(provider) => Some(provider),
            Writes::SecretOnly | Writes::Added => None,
        }
    }

    pub fn email(&self) -> &str {
        &self.config.email
    }

    pub const fn provider(&self) -> Provider {
        self.config.provider
    }

    /// Writes the secret, then the config when it changed.
    pub fn save(self, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = dir.join("config.toml");
        let config = &self.config;
        // The secret goes first, so a config never names an Account without one.
        config
            .secrets
            .write(dir, &config.email, self.sign_in.secret())?;

        // Before the config, so the Account never opens the old Provider's Cache.
        if matches!(self.writes, Writes::Moved(_)) {
            cache::delete(&xdg::cache_dir()?)?;
        }

        if !matches!(self.writes, Writes::SecretOnly) {
            let text = self.partial.with(&config.email, config.provider);
            atomic_file::write(&config_path, text.as_bytes(), 0o644)?;
        }

        Ok(())
    }
}
