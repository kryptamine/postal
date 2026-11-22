use std::path::Path;

use super::Writes;
use crate::{
    account::config::Config,
    account::config_file::ConfigFile,
    account::provider::Provider,
    account::sign_in::{SignIn, smtp_key},
    account::xdg,
    cache,
};

/// A sign-in the IMAP server accepted, not yet written.
pub struct Granted {
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
        let secrets = config.secrets;
        secrets.write(dir, &config.email, self.sign_in.secret())?;
        let smtp = smtp_key(&config.email);
        // A blank SMTP password keeps the one stored, while SMTP has a username.
        match self.sign_in.smtp_secret() {
            Some(secret) => secrets.write(dir, &smtp, secret)?,
            None if config.smtp.username.is_none() => secrets.delete(dir, &smtp)?,
            None => {}
        }

        // Before the config, so the Account never opens the old Provider's Cache.
        if matches!(self.writes, Writes::Moved(_)) {
            cache::delete(&xdg::cache_dir()?)?;
        }

        // The servers of a generic Provider may have been typed anew.
        let own_servers = config.provider == Provider::Other;
        if own_servers || !matches!(self.writes, Writes::SecretOnly) {
            let mut file = ConfigFile::load(&config_path)?;
            file.set_account(&config.email, config.provider);
            if own_servers {
                file.set_servers(&config.email, &config.imap, &config.smtp);
            }

            file.save()?;
        }

        Ok(())
    }
}
