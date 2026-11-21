use std::io;

use crate::{account::oauth, mail::outage::Outage};

/// What a server's answer holds, lowercased, when it is busy rather than
/// refusing Postal. async-imap knows none of these codes and leaves them in
/// the response text.
const BUSY: [&str; 3] = ["[throttled]", "[unavailable]", "bandwidth"];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot connect to {address}: {source}")]
    Connect { address: String, source: io::Error },
    #[error("the IMAP server closed the connection")]
    Closed,
    /// The server said `BYE`, with this text.
    #[error("the IMAP server said BYE: {0}")]
    Bye(String),
    #[error("IMAP: {0}")]
    Imap(#[from] async_imap::error::Error),
    /// No access token to sign in with.
    #[error(transparent)]
    Token(#[from] oauth::Error),
}

impl Error {
    /// The outage this error stands for; `None` when retrying would fail
    /// the same way, as a refused login does.
    pub fn outage(&self) -> Option<Outage> {
        use async_imap::error::Error as Imap;
        match self {
            Self::Connect { .. }
            | Self::Closed
            | Self::Imap(Imap::Io(_) | Imap::ConnectionLost)
            | Self::Token(oauth::Error::Http(_)) => Some(Outage::Offline),
            Self::Bye(_) => Some(Outage::Busy),
            Self::Imap(Imap::No(text) | Imap::Bad(text)) => {
                let text = text.to_ascii_lowercase();
                let busy = BUSY.iter().any(|marker| text.contains(marker));
                busy.then_some(Outage::Busy)
            }
            Self::Imap(_) | Self::Token(_) => None,
        }
    }

    /// The server refused the access token, as RFC 5530's
    /// `AUTHENTICATIONFAILED` says; async-imap leaves the code in the text.
    pub fn bad_credentials(&self) -> bool {
        matches!(self, Self::Imap(async_imap::error::Error::No(text))
            if text.to_ascii_lowercase().contains("[authenticationfailed]"))
    }

    /// Only a new login helps: the server refused the credentials, or Google
    /// the refresh token.
    pub fn signed_out(&self) -> bool {
        self.bad_credentials() || matches!(self, Self::Token(token) if token.signed_out())
    }

    /// The server's own words when it answered `NO` to this command alone,
    /// leaving the connection usable; `None` for any other error.
    pub fn refusal(&self) -> Option<&str> {
        let Self::Imap(async_imap::error::Error::No(text)) = self else {
            return None;
        };

        if self.outage().is_some() {
            return None;
        }

        // async-imap writes `code: .., info: Some("<text>")` around it.
        let info = text
            .split_once("info: Some(\"")
            .and_then(|(_, rest)| rest.strip_suffix("\")"));
        Some(info.unwrap_or(text))
    }
}
