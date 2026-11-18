use std::{fmt, path::Path, time::Duration};

use crate::{
    atomic_file, browser,
    config::{Config, SecretStore},
    credential::Credential,
    imap::{self, ImapSession, MailServer},
    keychain,
    partial_config::PartialConfig,
    prompt, secret,
};

const APP_PASSWORDS: &str = "https://myaccount.google.com/apppasswords";

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

/// The Onboarding with an app password: asks until the server accepts one,
/// and writes nothing before it does.
pub fn app_password(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = dir.join("config.toml");
    let partial = PartialConfig::load(&config_path)?;
    let email = ask("Email:", true)?;
    let known = match partial.email.as_deref() {
        Some(current) if current.eq_ignore_ascii_case(&email) => true,
        Some(current) => {
            let current = current.to_owned();
            return Err(Error::OtherAccount { current }.into());
        }
        None => false,
    };

    println!(
        "Google gives app passwords only to an Account with 2-Step Verification.\n\
         Create one for Postal at {APP_PASSWORDS}"
    );
    if let Err(error) = browser::open(APP_PASSWORDS) {
        tracing::warn!("cannot open the browser: {error}");
    }

    let config = partial.with_email(email);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let password = loop {
        let pasted = ask("App password:", false)?;
        let password = Credential::new(pasted.split_whitespace().collect());
        match runtime.block_on(verify(&config, &password)) {
            Ok(()) => break password,
            Err(Error::Imap(error)) if error.bad_credentials() => {
                println!("The server refused this app password; paste it again.");
            }
            Err(error) => return Err(error.into()),
        }
    };

    // The secret goes first, so a config never names an Account without one.
    match config.secrets {
        SecretStore::Keychain => keychain::write(&config.email, &password, &config_path)?,
        SecretStore::File => secret::write(&dir.join("secret"), &password)?,
    }

    if !known {
        atomic_file::write(&config_path, config.to_toml().as_bytes(), 0o644)?;
    }

    println!("Signed in as {}.", config.email);
    Ok(())
}

/// Prompts until the answer holds more than whitespace.
fn ask(label: &str, echo: bool) -> Result<String, Box<dyn std::error::Error>> {
    loop {
        let line = prompt::read_line(label, echo)?.ok_or(Error::Cancelled)?;
        if !line.trim().is_empty() {
            return Ok(line.trim().to_owned());
        }
    }
}

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
