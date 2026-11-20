use std::{error::Error, path::Path};

use crate::{atomic_file, cache, partial_config::PartialConfig, xdg};

/// Takes the Account of `email` out of `config.toml` in `dir`, then deletes
/// its Cache rows and its secret: a config never names it without them.
/// The secret goes last, as a locked keychain refuses it most often.
pub fn unlink(dir: &Path, email: &str) -> Result<(), Box<dyn Error>> {
    let path = dir.join("config.toml");
    let mut config = PartialConfig::load(&path)?;
    config.remove(email);
    atomic_file::write(&path, config.to_toml().as_bytes(), 0o644)?;
    cache::forget(&xdg::cache_dir()?, email)?;
    config.secrets.delete(dir, email)?;
    tracing::info!("unlinked {email}");
    Ok(())
}
