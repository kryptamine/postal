use std::{
    fmt::{self, Write},
    io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{oauth::Endpoints, partial_config::PartialConfig, provider::Provider};

/// The Account settings read from `config.toml`.
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
    /// `None` while the file is missing or names no Account yet.
    pub fn load(path: &Path) -> Result<Option<Self>, Error> {
        let mut partial = PartialConfig::load(path)?;
        let provider = partial.provider;
        Ok(partial
            .email
            .take()
            .map(|email| partial.account(email, provider)))
    }

    /// The file `load` reads back as this Config; the defaults are left out.
    pub fn to_toml(&self) -> String {
        let mut text = format!("email = {}\n", toml::Value::from(self.email.as_str()));
        if self.provider != Provider::default() {
            writeln!(text, "provider = \"{}\"", self.provider.key()).ok();
        }

        if let Some(name) = &self.name {
            writeln!(text, "name = {}", toml::Value::from(name.as_str())).ok();
        }

        match self.icons {
            IconChoice::Auto => {}
            IconChoice::Nerd => text += "icons = \"nerd\"\n",
            IconChoice::None => text += "icons = \"none\"\n",
        }

        if self.imap != Imap::of(self.provider) {
            let host = toml::Value::from(self.imap.host.as_str());
            let Imap { port, tls, .. } = self.imap;
            // Writing to a String cannot fail.
            write!(
                text,
                "\n[imap]\nhost = {host}\nport = {port}\ntls = {tls}\n"
            )
            .ok();
        }

        if self.smtp != Smtp::of(self.provider) {
            let host = toml::Value::from(self.smtp.host.as_str());
            let Smtp {
                port,
                tls,
                starttls,
                ..
            } = self.smtp;
            write!(
                text,
                "\n[smtp]\nhost = {host}\nport = {port}\ntls = {tls}\nstarttls = {starttls}\n"
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
