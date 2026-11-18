use std::{
    fmt::{self, Write},
    io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::partial_config::PartialConfig;

/// The Account settings read from `config.toml`.
pub struct Config {
    pub email: String,
    pub imap: Imap,
    pub secrets: SecretStore,
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
    Missing {
        path: PathBuf,
        field: &'static str,
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
            Self::Missing { path, field } => {
                write!(f, "{}: missing field `{field}`", path.display())
            }
        }
    }
}

impl std::error::Error for Error {}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Read {
            path: path.to_owned(),
            source,
        })?;
        PartialConfig::parse(path, &text)?
            .config()
            .map_err(|field| Error::Missing {
                path: path.to_owned(),
                field,
            })
    }

    /// The file `load` reads back as this Config; the defaults are left out.
    pub fn to_toml(&self) -> String {
        let mut text = format!(
            "email = {}\nauth = \"app-password\"\n",
            toml::Value::from(self.email.as_str())
        );
        if self.imap != Imap::gmail() {
            let host = toml::Value::from(self.imap.host.as_str());
            let Imap { port, tls, .. } = self.imap;
            // Writing to a String cannot fail.
            let _ = write!(
                text,
                "\n[imap]\nhost = {host}\nport = {port}\ntls = {tls}\n"
            );
        }

        if matches!(self.secrets, SecretStore::File) {
            text += "\n[secrets]\nstore = \"file\"\n";
        }

        text
    }
}
