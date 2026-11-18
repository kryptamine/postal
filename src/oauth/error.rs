use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// The token endpoint could not be reached or broke off.
    Http(String),
    /// The token endpoint answered with an OAuth error code.
    Refused {
        error: String,
        description: Option<String>,
    },
    /// An answer Postal cannot read.
    Invalid(String),
}

impl Error {
    /// The refresh token was revoked or expired: only a new login helps.
    pub fn signed_out(&self) -> bool {
        matches!(self, Self::Refused { error, .. } if error == "invalid_grant")
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(reason) => write!(f, "cannot reach Google's token endpoint: {reason}"),
            Self::Refused { .. } if self.signed_out() => f.write_str(
                "Google no longer accepts this sign-in (invalid_grant); sign in again with `postal login`",
            ),
            Self::Refused {
                error,
                description: Some(description),
            } => write!(f, "Google refused the token request: {error}: {description}"),
            Self::Refused { error, .. } => write!(f, "Google refused the token request: {error}"),
            Self::Invalid(reason) => write!(f, "unreadable answer from Google's token endpoint: {reason}"),
        }
    }
}

impl std::error::Error for Error {}
