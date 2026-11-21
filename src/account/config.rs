use std::{
    io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

pub use crate::account::secret_store::SecretStore;
use crate::{
    account::oauth::Endpoints, account::partial_config::PartialConfig, account::provider::Provider,
};

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

/// Whether to draw Nerd Font icons: `auto` guesses from the terminal.
#[derive(Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IconChoice {
    #[default]
    Auto,
    Nerd,
    None,
}

impl IconChoice {
    /// The value of `icons` in `config.toml`.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Nerd => "nerd",
            Self::None => "none",
        }
    }

    /// The one after this, back to `auto` after `none`.
    pub const fn next(self) -> Self {
        match self {
            Self::Auto => Self::Nerd,
            Self::Nerd => Self::None,
            Self::None => Self::Auto,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: {source}", .path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("{}:{line}:{column}: {message}", .path.display())]
    Invalid {
        path: PathBuf,
        line: usize,
        column: usize,
        message: String,
    },
}

impl Error {
    /// `message` about byte `offset` of the file's `text`.
    pub fn invalid(path: &Path, text: &str, offset: usize, message: &str) -> Self {
        let before = text.get(..offset).unwrap_or(text);
        let line = before.rsplit('\n').next().unwrap_or_default();
        Self::Invalid {
            path: path.to_owned(),
            line: before.matches('\n').count() + 1,
            // Characters, not bytes.
            column: line.chars().count() + 1,
            message: message.trim_end().to_owned(),
        }
    }
}

impl Config {
    /// Every Account the file names; none while it is missing.
    pub fn load(path: &Path) -> Result<Vec<Self>, Error> {
        Ok(PartialConfig::load(path)?.configs())
    }
}
