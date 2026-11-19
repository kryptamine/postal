use std::{
    fmt::{self, Write},
    io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{oauth::Endpoints, partial_config::PartialConfig};

/// The Account settings read from `config.toml`.
pub struct Config {
    pub email: String,
    /// The display name of `From`; without it, the bare address.
    pub name: Option<String>,
    pub imap: Imap,
    pub smtp: Smtp,
    pub secrets: SecretStore,
    pub oauth: Endpoints,
}

#[derive(PartialEq, Eq)]
pub struct Imap {
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

impl Imap {
    pub fn gmail() -> Self {
        Self {
            host: "imap.gmail.com".to_owned(),
            port: 993,
            tls: true,
        }
    }
}

/// Implicit TLS only: there is no STARTTLS.
#[derive(PartialEq, Eq)]
pub struct Smtp {
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

impl Smtp {
    pub fn gmail() -> Self {
        Self {
            host: "smtp.gmail.com".to_owned(),
            port: 465,
            tls: true,
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
    /// `None` while the file is missing or names no Account yet.
    pub fn load(path: &Path) -> Result<Option<Self>, Error> {
        let mut partial = PartialConfig::load(path)?;
        Ok(partial.email.take().map(|email| partial.account(email)))
    }

    /// The file `load` reads back as this Config; the defaults are left out.
    pub fn to_toml(&self) -> String {
        let mut text = format!("email = {}\n", toml::Value::from(self.email.as_str()));
        if let Some(name) = &self.name {
            writeln!(text, "name = {}", toml::Value::from(name.as_str())).ok();
        }

        if self.imap != Imap::gmail() {
            let host = toml::Value::from(self.imap.host.as_str());
            let Imap { port, tls, .. } = self.imap;
            // Writing to a String cannot fail.
            write!(
                text,
                "\n[imap]\nhost = {host}\nport = {port}\ntls = {tls}\n"
            )
            .ok();
        }

        if self.smtp != Smtp::gmail() {
            let host = toml::Value::from(self.smtp.host.as_str());
            let Smtp { port, tls, .. } = self.smtp;
            write!(
                text,
                "\n[smtp]\nhost = {host}\nport = {port}\ntls = {tls}\n"
            )
            .ok();
        }

        if matches!(self.secrets, SecretStore::File) {
            text += "\n[secrets]\nstore = \"file\"\n";
        }

        if self.oauth != Endpoints::google() {
            let Endpoints {
                authorization_url,
                token_url,
                redirect_timeout,
            } = &self.oauth;

            write!(
                text,
                "\n[oauth]\nauthorization-url = {}\ntoken-url = {}\nredirect-timeout = {}\n",
                toml::Value::from(authorization_url.as_str()),
                toml::Value::from(token_url.as_str()),
                redirect_timeout.as_secs()
            )
            .ok();
        }

        text
    }
}
