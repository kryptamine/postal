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

/// The Account `account` names, else every one.
pub fn named(account: Option<&str>) -> Result<Vec<String>, Box<dyn Error>> {
    let emails = emails()?;
    let Some(account) = account.map(str::to_lowercase) else {
        return Ok(emails);
    };

    if !emails.contains(&account) {
        return Err(format!("no Account {account} is set up").into());
    }

    Ok(vec![account])
}
