use std::path::Path;

use super::{Error, Granted, verify, writes};
use crate::{
    credential::Credential, partial_config::PartialConfig, provider::Provider, sign_in::SignIn,
};

/// Signs in to `provider`'s IMAP server as `email` with an app-specific
/// `password`; nothing is written before it accepts them.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn check_password(
    dir: &Path,
    provider: Provider,
    (email, password): (String, Credential),
) -> Result<Granted, Box<dyn std::error::Error>> {
    let partial = PartialConfig::load(&dir.join("config.toml"))?;
    let writes = writes(&partial, &email, provider);
    let config = partial.account(email, provider);
    let sign_in = SignIn::Password(password);
    match verify(&config, &sign_in).await {
        Ok(()) => {}
        Err(Error::Imap(error)) if error.bad_credentials() => {
            return Err(Error::PasswordRefused(provider).into());
        }
        Err(error) => return Err(error.into()),
    }

    Ok(Granted {
        config,
        sign_in,
        writes,
    })
}
