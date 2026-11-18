mod app_password;
mod oauth;

use std::{fmt, io, path::Path, time::Duration};

pub use self::{app_password::app_password, oauth::oauth};
use crate::{
    atomic_file,
    config::{Auth, Config, SecretStore},
    imap::{self, ImapSession, MailServer},
    keychain,
    partial_config::PartialConfig,
    secret,
    sign_in::SignIn,
};

/// For connect, sign-in and `EXAMINE` together.
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
            Self::Denied(code) if code == "access_denied" || code == "admin_policy_enforced" => {
                write!(
                    f,
                    "Google refused the sign-in ({code}); \
                     sign in with an app password instead: `postal login --app-password`"
                )
            }
            Self::Denied(code) => write!(f, "Google refused the sign-in: {code}"),
            Self::NoRedirect(waited) => write!(
                f,
                "the browser did not come back within {}s; run `postal login` again",
                waited.as_secs()
            ),
            Self::Loopback(source) => {
                write!(f, "the sign-in listener on 127.0.0.1 failed: {source}")
            }
            Self::NoMailScope => f.write_str(
                "Google did not grant access to Gmail; run `postal login` again \
                 and leave the Gmail permission ticked",
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Whether the config already names this Account with this `auth`, and is
/// then left alone; naming another Account is an error.
fn unchanged(partial: &PartialConfig, email: &str, auth: Auth) -> Result<bool, Error> {
    match partial.email.as_deref() {
        Some(current) if current.eq_ignore_ascii_case(email) => Ok(partial.auth == Some(auth)),
        Some(current) => Err(Error::OtherAccount {
            current: current.to_owned(),
        }),
        None => Ok(false),
    }
}

#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn verify(config: &Config, sign_in: &SignIn) -> Result<(), Error> {
    let verified = async {
        let mut session = ImapSession::open(config, sign_in).await?;
        session.examine().await?;
        Ok(())
    };

    tokio::time::timeout(VERIFY_TIMEOUT, verified)
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(Error::Imap)
}

/// Writes the secret, then the config unless it is `unchanged`.
fn save(
    dir: &Path,
    config: &Config,
    sign_in: &SignIn,
    unchanged: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = dir.join("config.toml");
    // The secret goes first, so a config never names an Account without one.
    match config.secrets {
        SecretStore::Keychain => keychain::write(&config.email, sign_in.secret(), &config_path)?,
        SecretStore::File => secret::write(&dir.join("secret"), sign_in.secret())?,
    }

    if !unchanged {
        atomic_file::write(&config_path, config.to_toml().as_bytes(), 0o644)?;
    }

    println!("Signed in as {}.", config.email);
    Ok(())
}
