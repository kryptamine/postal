use crate::{account::credential::Credential, account::oauth::Tokens};

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
