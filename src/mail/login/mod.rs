mod consent;
mod granted;
mod password;

use std::{io, time::Duration};

pub use self::{consent::Consent, granted::Granted, password::check_password};
use crate::{
    account::config::Config,
    account::partial_config::PartialConfig,
    account::provider::Provider,
    account::sign_in::SignIn,
    domain::folder,
    mail::imap::{self, ImapSession, MailServer},
};

/// For connect, sign-in and `EXAMINE` together.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("login cancelled")]
    Cancelled,
    #[error("the IMAP server did not answer within {}s", VERIFY_TIMEOUT.as_secs())]
    Timeout,
    #[error(transparent)]
    Imap(#[from] imap::Error),
    /// Google's consent page sent back this `error` code.
    #[error("Google refused the sign-in: {0}")]
    Denied(String),
    /// The browser did not come back in time.
    #[error("the browser did not come back within {}s", .0.as_secs())]
    NoRedirect(Duration),
    #[error("the sign-in listener on 127.0.0.1 failed: {0}")]
    Loopback(io::Error),
    /// The user unticked the Gmail permission on the consent screen.
    #[error("Google did not grant access to Gmail; leave the Gmail permission ticked")]
    NoMailScope,
    /// The IMAP server refused the address or the password.
    #[error("{} refused the address or the app-specific password", .0.service())]
    PasswordRefused(Provider),
}

/// What a login writes besides the secret.
enum Writes {
    /// The config names this Account already.
    SecretOnly,
    /// The Account is added to the config.
    Added,
    /// The config names this address at this other Provider, whose Cache
    /// goes with it.
    Moved(Provider),
}

/// The same address at another Provider is another Account, with another Cache.
fn writes(partial: &PartialConfig, email: &str, provider: Provider) -> Writes {
    match partial.provider_of(email) {
        Some(written) if written == provider => Writes::SecretOnly,
        Some(written) => Writes::Moved(written),
        None => Writes::Added,
    }
}

async fn verify(config: &Config, sign_in: &SignIn) -> Result<(), Error> {
    let verified = async {
        let mut session = ImapSession::open(config, sign_in).await?;
        session.examine(folder::INBOX).await?;
        Ok::<(), Error>(())
    };

    tokio::time::timeout(VERIFY_TIMEOUT, verified)
        .await
        .map_err(|_elapsed| Error::Timeout)?
}
