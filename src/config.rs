use std::{
    fmt, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{oauth::Endpoints, partial_config::PartialConfig, provider::Provider};

/// One Account's settings from `config.toml`, with those every Account shares.
pub struct Config {
    pub email: String,
    pub provider: Provider,
    /// The display name of `From`; without it, the bare address.
    pub name: Option<String>,
    pub imap: Imap,
    pub smtp: Smtp,
    pub secrets: SecretStore,
    pub oauth: Endpoints,
    pub icons: IconChoice,
}

#[derive(PartialEq, Eq)]
pub struct Imap {
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

impl Imap {
    pub fn of(provider: Provider) -> Self {
        let host = match provider {
            Provider::Google => "imap.gmail.com",
            Provider::ICloud => "imap.mail.me.com",
        };

        Self {
            host: host.to_owned(),
            port: 993,
            tls: true,
        }
    }
}

/// With `tls`, implicit TLS, or STARTTLS after a plaintext start when
/// `starttls` is set too.
#[derive(PartialEq, Eq)]
pub struct Smtp {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub starttls: bool,
}

impl Smtp {
    /// iCloud's SMTP takes STARTTLS on 587 alone (support.apple.com/102525).
    pub fn of(provider: Provider) -> Self {
        match provider {
            Provider::Google => Self {
                host: "smtp.gmail.com".to_owned(),
                port: 465,
                tls: true,
                starttls: false,
            },
            Provider::ICloud => Self {
                host: "smtp.mail.me.com".to_owned(),
                port: 587,
                tls: true,
                starttls: true,
            },
        }
    }
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum SecretStore {
    #[default]
    Keychain,
    File,
}

/// Whether to draw Nerd Font icons: `auto` guesses from the terminal.
#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum IconChoice {
    #[default]
    Auto,
    Nerd,
    None,
}

#[derive(Debug)]
pub enum Error {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Invalid {
        path: PathBuf,
        line: usize,
        column: usize,
        message: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Invalid {
                path,
                line,
                column,
                message,
            } => write!(f, "{}:{line}:{column}: {message}", path.display()),
        }
    }
}

impl std::error::Error for Error {}

impl Config {
    /// Every Account the file names; none while it is missing.
    pub fn load(path: &Path) -> Result<Vec<Self>, Error> {
        Ok(PartialConfig::load(path)?.configs())
    }
}
