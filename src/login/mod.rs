mod app_password;

use std::{fmt, time::Duration};

pub use self::app_password::app_password;
use crate::{
    config::Config,
    credential::Credential,
    imap::{self, ImapSession, MailServer},
};

/// For connect, `LOGIN` and `EXAMINE` together.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub enum Error {
    /// The config already holds another Account.
    OtherAccount {
        current: String,
    },
    Cancelled,
    Timeout,
    Imap(imap::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OtherAccount { current } => write!(
                f,
                "already signed in as {current}; signing in as another Account is not supported"
            ),
            Self::Cancelled => f.write_str("login cancelled"),
            Self::Timeout => write!(
                f,
                "the IMAP server did not answer within {}s",
                VERIFY_TIMEOUT.as_secs()
            ),
            Self::Imap(source) => source.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

async fn verify(config: &Config, password: &Credential) -> Result<(), Error> {
    let verified = async {
        let mut session = ImapSession::open(config, password).await?;
        session.examine().await?;
        Ok(())
    };

    tokio::time::timeout(VERIFY_TIMEOUT, verified)
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(Error::Imap)
}
