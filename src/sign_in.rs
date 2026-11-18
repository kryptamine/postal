use crate::{credential::Credential, oauth::Tokens};

/// How Postal proves the Account to the IMAP server.
pub enum SignIn {
    /// `LOGIN` with an app password.
    Password(Credential),
    /// `AUTHENTICATE XOAUTH2` with an access token.
    OAuth(Tokens),
}

impl SignIn {
    /// What the secret store keeps for it.
    pub const fn secret(&self) -> &Credential {
        match self {
            Self::Password(password) => password,
            Self::OAuth(tokens) => tokens.refresh_token(),
        }
    }
}
