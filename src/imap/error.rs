use std::{fmt, io};

#[derive(Debug)]
pub enum Error {
    Connect { address: String, source: io::Error },
    Closed,
    Imap(async_imap::error::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect { address, source } => write!(f, "cannot connect to {address}: {source}"),
            Self::Closed => f.write_str("the IMAP server closed the connection"),
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
