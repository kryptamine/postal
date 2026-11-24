use std::{error::Error, path::Path};

use crate::{
    account::config::Config, account::credential::Credential, account::oauth::Tokens,
    account::provider::Provider,
};

/// How the Account signs in to its servers, as its Provider decides.
pub enum SignIn {
    OAuth(Tokens),
    /// A password, for a Provider with no OAuth (ADR 0003); SMTP takes
    /// `smtp` when the server wants one of its own.
    Password {
        password: Credential,
        smtp: Option<Credential>,
    },
}

impl SignIn {
    /// How `config` signs in, from its secret; `None` once that is lost.
    pub fn stored(config: &Config, dir: &Path) -> Result<Option<Self>, Box<dyn Error>> {
        let Some(secret) = config.secrets.read(dir, &config.email)? else {
            return Ok(None);
        };

        Ok(Some(match config.provider {
            Provider::ICloud | Provider::Other => Self::Password {
                password: secret,
                smtp: config.secrets.read(dir, &smtp_key(&config.email))?,
            },
            Provider::Google => Self::OAuth(Tokens::stored(secret, config.oauth.token_url.clone())),
        }))
    }

    /// What the secret store keeps: the refresh token, or the password.
    pub const fn secret(&self) -> &Credential {
        match self {
            Self::OAuth(tokens) => tokens.refresh_token(),
            Self::Password { password, .. } => password,
        }
    }

    /// The SMTP password the secret store keeps under `smtp_key`, if any.
    pub const fn smtp_secret(&self) -> Option<&Credential> {
        match self {
            Self::Password {
                smtp: Some(smtp), ..
            } => Some(smtp),
            Self::OAuth(_) | Self::Password { smtp: None, .. } => None,
        }
    }
}

/// Where the secret store keeps the SMTP password of `email`, apart from
/// its own secret.
pub fn smtp_key(email: &str) -> String {
    format!("smtp:{email}")
}
