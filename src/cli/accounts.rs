use std::error::Error;

use crate::{account::config::Config, account::xdg};

/// Every Account's address, lowercased as the Cache keys it, in the order
/// `config.toml` lists them.
pub fn emails() -> Result<Vec<String>, Box<dyn Error>> {
    let configs = Config::load(&xdg::config_dir()?.join("config.toml"))?;
    Ok(configs
        .into_iter()
        .map(|c| c.email.to_lowercase())
        .collect())
}
