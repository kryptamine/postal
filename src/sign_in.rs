use crate::{credential::Credential, oauth::Tokens};

/// How the Account signs in to its servers, as its Provider decides.
pub enum SignIn {
    OAuth(Tokens),
    /// An app-specific password, for a Provider with no OAuth (ADR 0003).
    Password(Credential),
}

impl SignIn {
    /// What the secret store keeps: the refresh token, or the password.
    pub const fn secret(&self) -> &Credential {
        match self {
            Self::OAuth(tokens) => tokens.refresh_token(),
            Self::Password(password) => password,
        }
    }
}
