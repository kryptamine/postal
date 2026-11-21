use std::{error::Error, path::Path};

use crate::{
    account::config_file::ConfigFile, account::partial_config::PartialConfig, account::xdg, cache,
};

/// Takes the Account of `email` out of `config.toml` in `dir`, then deletes
/// its Cache rows and its secret: a config never names it without them.
/// The secret goes last, as a locked keychain refuses it most often.
pub fn unlink(dir: &Path, email: &str) -> Result<(), Box<dyn Error>> {
    let path = dir.join("config.toml");
    let config = PartialConfig::load(&path)?;
    let mut file = ConfigFile::load(&path)?;
    file.remove_account(email);
    file.save()?;
    cache::forget(&xdg::cache_dir()?, email)?;
    config.secrets.delete(dir, email)?;
    tracing::info!("unlinked {email}");
    Ok(())
}
