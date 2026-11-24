use std::{error::Error, path::Path};

use crate::{
    account::config::Config,
    account::sign_in::SignIn,
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
            if let Some(sign_in) = SignIn::stored(&config, dir)? {
                accounts.push((config, sign_in));
                break;
            }

            let (tty, background) = slot.take()?;
            let theme = onboarding_theme(dir, background)?;
            match onboarding::run(tty.terminal, tty.events, &theme, dir, Start::Again(&config))
                .await?
            {
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
