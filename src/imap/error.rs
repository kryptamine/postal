use std::{fmt, io};

use crate::outage::Outage;

/// What a server's answer holds, lowercased, when it is busy rather than
/// refusing Postal. async-imap knows none of these codes and leaves them in
/// the response text.
const BUSY: [&str; 3] = ["[throttled]", "[unavailable]", "bandwidth"];

#[derive(Debug)]
pub enum Error {
    Connect {
        address: String,
        source: io::Error,
    },
    Closed,
    /// The server said `BYE`, with this text.
    Bye(String),
    Imap(async_imap::error::Error),
}

impl Error {
    /// The outage this error stands for; `None` when retrying would fail
    /// the same way, as a refused login does.
    pub fn outage(&self) -> Option<Outage> {
        use async_imap::error::Error as Imap;
        match self {
            Self::Connect { .. }
            | Self::Closed
            | Self::Imap(Imap::Io(_) | Imap::ConnectionLost) => Some(Outage::Offline),
            Self::Bye(_) => Some(Outage::Busy),
            Self::Imap(Imap::No(text) | Imap::Bad(text)) => {
                let text = text.to_ascii_lowercase();
                let busy = BUSY.iter().any(|marker| text.contains(marker));
                busy.then_some(Outage::Busy)
            }
            Self::Imap(_) => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect { address, source } => write!(f, "cannot connect to {address}: {source}"),
            Self::Closed => f.write_str("the IMAP server closed the connection"),
            Self::Bye(text) => write!(f, "the IMAP server said BYE: {text}"),
            Self::Imap(source) => write!(f, "IMAP: {source}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<async_imap::error::Error> for Error {
    fn from(source: async_imap::error::Error) -> Self {
        Self::Imap(source)
    }
}
