use std::{
    fmt, io,
    net::IpAddr,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use toml::Spanned;

/// The Account settings read from `config.toml`.
pub struct Config {
    pub email: String,
    pub imap: Imap,
    pub secrets: SecretStore,
}

pub struct Imap {
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    email: String,
    auth: Auth,
    #[serde(default)]
    imap: RawImap,
    #[serde(default)]
    secrets: Secrets,
}

// Only the hand-written variants exist so far; any other value is rejected
// with its line by serde.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Auth {
    AppPassword,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Secrets {
    #[serde(default)]
    store: SecretStore,
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum SecretStore {
    #[default]
    Keychain,
    File,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, default)]
struct RawImap {
    host: String,
    port: u16,
    tls: Spanned<bool>,
}

impl Default for RawImap {
    fn default() -> Self {
        Self {
            host: "imap.gmail.com".to_owned(),
            port: 993,
            tls: Spanned::new(0..0, true),
        }
    }
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
    pub fn load(path: &Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Read {
            path: path.to_owned(),
            source,
        })?;
        let invalid = |span: std::ops::Range<usize>, message: &str| {
            let (line, column) = line_and_column(&text, span.start);
            Error::Invalid {
                path: path.to_owned(),
                line,
                column,
                message: message.trim_end().to_owned(),
            }
        };

        let raw: RawConfig =
            toml::from_str(&text).map_err(|e| invalid(e.span().unwrap_or(0..0), e.message()))?;
        match raw.auth {
            Auth::AppPassword => {}
        }

        let tls = *raw.imap.tls.get_ref();
        if !tls && !is_loopback(&raw.imap.host) {
            return Err(invalid(
                raw.imap.tls.span(),
                "`tls = false` is allowed only for a loopback host",
            ));
        }

        Ok(Self {
            email: raw.email,
            imap: Imap {
                host: raw.imap.host,
                port: raw.imap.port,
                tls,
            },
            secrets: raw.secrets.store,
        })
    }
}

fn is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// Both are 1-based; the column counts characters, not bytes.
fn line_and_column(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset.min(text.len())];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    (
        before.matches('\n').count() + 1,
        before[line_start..].chars().count() + 1,
    )
}
