mod app_password;
mod oauth;

use std::{fmt, io, path::Path, time::Duration};

pub use self::{app_password::app_password, oauth::oauth};
use crate::{
    atomic_file, cache,
    config::{Auth, Config, SecretStore},
    folder,
    imap::{self, ImapSession, MailServer},
    keychain,
    partial_config::PartialConfig,
    prompt, secret,
    sign_in::SignIn,
    xdg,
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

pub fn cancelled(error: &(dyn std::error::Error + 'static)) -> bool {
    matches!(error.downcast_ref(), Some(Error::Cancelled))
}

/// The login flow by the `auth` of the Account `config` names, for it.
pub fn sign_in_again(dir: &Path, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    match config.auth {
        Auth::OAuth => oauth(dir, Some(&config.email)),
        Auth::AppPassword => app_password(dir, Some(&config.email)),
    }
}

/// What a login writes besides the secret.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Writes {
    /// The config names this Account with this `auth` already.
    SecretOnly,
    Config,
    /// The config named another Account, whose Cache goes with it.
    Account,
}

/// What a login as `email` writes; replacing another Account asks first.
fn writes(
    partial: &PartialConfig,
    email: &str,
    auth: Auth,
) -> Result<Writes, Box<dyn std::error::Error>> {
    match partial.email.as_deref() {
        Some(current) if current.eq_ignore_ascii_case(email) && partial.auth == Some(auth) => {
            Ok(Writes::SecretOnly)
        }
        Some(current) if !current.eq_ignore_ascii_case(email) => {
            let question = format!(
                "Signed in as {current}. Sign in as {email} instead and delete the Cache? [y/N]"
            );
            match prompt::read_line(&question, "", true)?
                .as_deref()
                .map(str::trim)
            {
                Some("y" | "Y" | "yes") => Ok(Writes::Account),
                _ => Err(Error::Cancelled.into()),
            }
        }
        _ => Ok(Writes::Config),
    }
}

#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn verify(config: &Config, sign_in: &SignIn) -> Result<(), Error> {
    let verified = async {
        let mut session = ImapSession::open(config, sign_in).await?;
        session.examine(folder::INBOX).await?;
        Ok(())
    };

    // Without raw mode Ctrl-C is SIGINT, which would end the TUI too.
    let verified = tokio::select! {
        verified = tokio::time::timeout(VERIFY_TIMEOUT, verified) => verified,
        _ = tokio::signal::ctrl_c() => return Err(Error::Cancelled),
    };

    verified.map_err(|_| Error::Timeout)?.map_err(Error::Imap)
}

/// Writes the secret, then what else `writes` says.
fn save(
    dir: &Path,
    config: &Config,
    sign_in: &SignIn,
    writes: Writes,
) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = dir.join("config.toml");
    // The secret goes first, so a config never names an Account without one.
    match config.secrets {
        SecretStore::Keychain => keychain::write(&config.email, sign_in.secret(), &config_path)?,
        SecretStore::File => secret::write(&dir.join("secret"), sign_in.secret())?,
    }

    // Before the config, so the new Account never opens the old one's Cache.
    if writes == Writes::Account {
        cache::delete(&xdg::cache_dir()?)?;
    }

    if writes != Writes::SecretOnly {
        atomic_file::write(&config_path, config.to_toml().as_bytes(), 0o644)?;
    }

    println!("Signed in as {}.", config.email);
    Ok(())
}
