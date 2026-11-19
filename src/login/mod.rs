mod consent;
mod granted;

use std::{fmt, io, time::Duration};

pub use self::{consent::Consent, granted::Granted};
use crate::{
    config::Config,
    folder,
    imap::{self, ImapSession, MailServer},
    oauth::Tokens,
    partial_config::PartialConfig,
};

/// For connect, sign-in and `EXAMINE` together.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub enum Error {
    Cancelled,
    Timeout,
    Imap(imap::Error),
    /// Google's consent page sent back this `error` code.
    Denied(String),
    /// The browser did not come back in time.
    NoRedirect(Duration),
    Loopback(io::Error),
    /// The user unticked the Gmail permission on the consent screen.
    NoMailScope,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("login cancelled"),
            Self::Timeout => write!(
                f,
                "the IMAP server did not answer within {}s",
                VERIFY_TIMEOUT.as_secs()
            ),
            Self::Imap(source) => source.fmt(f),
            Self::Denied(code) => write!(f, "Google refused the sign-in: {code}"),
            Self::NoRedirect(waited) => write!(
                f,
                "the browser did not come back within {}s",
                waited.as_secs()
            ),
            Self::Loopback(source) => {
                write!(f, "the sign-in listener on 127.0.0.1 failed: {source}")
            }
            Self::NoMailScope => f.write_str(
                "Google did not grant access to Gmail; leave the Gmail permission ticked",
            ),
        }
    }
}

impl std::error::Error for Error {}

/// What a login writes besides the secret.
enum Writes {
    /// The config names this Account already.
    SecretOnly,
    Config,
    /// The config named this other Account, whose Cache goes with it.
    Account(String),
}

fn writes(partial: &PartialConfig, email: &str) -> Writes {
    match &partial.email {
        Some(current) if current.eq_ignore_ascii_case(email) => Writes::SecretOnly,
        Some(current) => Writes::Account(current.clone()),
        None => Writes::Config,
    }
}

#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn verify(config: &Config, sign_in: &Tokens) -> Result<(), Error> {
    let verified = async {
        let mut session = ImapSession::open(config, sign_in).await?;
        session.examine(folder::INBOX).await?;
        Ok(())
    };

    tokio::time::timeout(VERIFY_TIMEOUT, verified)
        .await
        .map_err(|_elapsed| Error::Timeout)?
        .map_err(Error::Imap)
}
