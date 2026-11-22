use std::{error::Error, path::Path};

use crate::{
    account::config::Config,
    account::oauth::Tokens,
    account::provider::Provider,
    account::sign_in::{SignIn, smtp_key},
    app::onboard::{onboard, onboarding_theme},
    domain::per_account::PerAccount,
    mail::login,
    ui::onboarding::{self, Outcome, Start},
    ui::terminal_slot::TerminalSlot,
};

/// The Accounts the config names, each with its sign-in.
pub async fn opened(
    slot: &mut TerminalSlot,
    dir: &Path,
) -> Result<(PerAccount<Config>, PerAccount<Option<SignIn>>), Box<dyn Error>> {
    let (configs, sign_ins): (Vec<_>, Vec<_>) = signed_in(slot, dir).await?.into_iter().unzip();
    Ok((
        configs.into_iter().collect(),
        sign_ins.into_iter().map(Some).collect(),
    ))
}

/// The Accounts the TUI opens with; with none yet, the Onboarding runs
/// first. A lost secret shows it for that Account alone, and leaving it
/// runs without the Account, unless none is left.
async fn signed_in(
    slot: &mut TerminalSlot,
    dir: &Path,
) -> Result<Vec<(Config, SignIn)>, Box<dyn Error>> {
    let config_path = dir.join("config.toml");
    let mut configs = Config::load(&config_path)?;
    if configs.is_empty() {
        onboard(slot, dir, Start::Choose).await?;
        configs = Config::load(&config_path)?;
    }

    let mut accounts = Vec::new();
    let mut left = None;
    for config in configs {
        loop {
            if let Some(sign_in) = read_sign_in(&config, dir)? {
                accounts.push((config, sign_in));
                break;
            }

            let (terminal, background) = slot.take()?;
            let theme = onboarding_theme(dir, background)?;
            match onboarding::run(terminal, &theme, dir, Start::Again(&config)).await? {
                Outcome::SignedIn(_) => {}
                Outcome::Quit(error) => {
                    tracing::info!("running without {}", config.email);
                    left = error;
                    break;
                }
            }
        }
    }

    if accounts.is_empty() {
        return Err(left.unwrap_or_else(|| login::Error::Cancelled.into()));
    }

    Ok(accounts)
}

/// How `config` signs in, from its secret; `None` once that is lost.
pub fn read_sign_in(config: &Config, dir: &Path) -> Result<Option<SignIn>, Box<dyn Error>> {
    let Some(secret) = config.secrets.read(dir, &config.email)? else {
        return Ok(None);
    };

    Ok(Some(match config.provider {
        Provider::ICloud | Provider::Other => SignIn::Password {
            password: secret,
            smtp: config.secrets.read(dir, &smtp_key(&config.email))?,
        },
        Provider::Google => match Tokens::refreshed(secret, config.oauth.token_url.clone()) {
            Ok(tokens) => SignIn::OAuth(tokens),
            Err(error) if error.signed_out() => {
                tracing::warn!("{error}");
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        },
    }))
}
