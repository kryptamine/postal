use std::path::Path;

use super::{Error, Granted, verify, writes};
use crate::{
    account::config::{Imap, Smtp},
    account::credential::Credential,
    account::partial_config::PartialConfig,
    account::provider::Provider,
    account::sign_in::SignIn,
};

/// The servers typed for a generic Provider, and an SMTP password when it
/// is not the IMAP one.
pub struct Servers {
    pub imap: Imap,
    pub smtp: Smtp,
    pub smtp_password: Option<Credential>,
}

/// Signs in to `provider`'s IMAP server, or to `servers`, as `email` with
/// `password`; nothing is written before it accepts them.
pub async fn check_password(
    dir: &Path,
    provider: Provider,
    email: String,
    password: Credential,
    servers: Option<Servers>,
) -> Result<Granted, Box<dyn std::error::Error>> {
    let partial = PartialConfig::load(&dir.join("config.toml"))?;
    let writes = writes(&partial, &email, provider);
    let mut config = partial.account(email, provider);
    let smtp = match servers {
        Some(servers) => {
            config.imap = servers.imap;
            config.smtp = servers.smtp;
            servers.smtp_password
        }
        None => None,
    };

    let sign_in = SignIn::Password { password, smtp };
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
