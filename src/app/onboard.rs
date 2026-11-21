use std::{error::Error, path::Path};

use crate::{
    account::config::Config,
    account::partial_config::PartialConfig,
    account::sign_in::SignIn,
    app::signed_in::read_sign_in,
    mail::login,
    render::icons::Icons,
    render::rgb::Rgb,
    ui::onboarding::{self, Outcome, Start},
    ui::terminal_slot::TerminalSlot,
    ui::theme::Theme,
};

/// The Theme before there is an Account: the config may set the icons.
pub fn onboarding_theme(dir: &Path, background: Rgb) -> Result<Theme, Box<dyn Error>> {
    let icons = PartialConfig::load(&dir.join("config.toml"))?.icons;
    Ok(Theme::from_env(background, Icons::choose(icons)))
}

/// The Onboarding from `start`; leaving it is an error, the last failure
/// or a cancel.
pub async fn onboard(
    slot: &mut TerminalSlot,
    dir: &Path,
    start: Start<'_>,
) -> Result<String, Box<dyn Error>> {
    let (terminal, background) = slot.take()?;
    let theme = onboarding_theme(dir, background)?;
    match onboarding::run(terminal, &theme, dir, start).await? {
        Outcome::SignedIn(email) => Ok(email),
        Outcome::Quit(error) => Err(error.unwrap_or_else(|| login::Error::Cancelled.into())),
    }
}

/// The Onboarding for `config` alone; what failed, if it did.
pub async fn sign_in_again(
    slot: &mut TerminalSlot,
    dir: &Path,
    config: &Config,
    sign_in: &mut Option<SignIn>,
) -> Result<Option<&'static str>, Box<dyn Error>> {
    let (terminal, background) = slot.take()?;
    let theme = onboarding_theme(dir, background)?;
    let failed = match onboarding::run(terminal, &theme, dir, Start::again(config)).await? {
        Outcome::SignedIn(_) => {
            *sign_in = read_sign_in(config, dir)?;
            sign_in.is_none()
        }
        Outcome::Quit(error) => {
            *sign_in = None;
            error.is_some()
        }
    };

    Ok(failed.then_some("sign-in failed"))
}
