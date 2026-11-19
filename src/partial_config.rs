use std::{io, net::IpAddr, ops::Range, path::Path, time::Duration};

use serde::Deserialize;
use toml::Spanned;

use crate::{
    config::{Config, Error, IconChoice, Imap, SecretStore, Smtp},
    oauth::Endpoints,
};

/// `config.toml` as login reads it: everything but the Account may be set
/// already, and a missing file is the Gmail defaults.
pub struct PartialConfig {
    pub email: Option<String>,
    pub name: Option<String>,
    pub imap: Imap,
    pub smtp: Smtp,
    pub secrets: SecretStore,
    pub oauth: Endpoints,
    pub icons: IconChoice,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    email: Option<String>,
    name: Option<String>,
    #[serde(default)]
    imap: RawImap,
    #[serde(default)]
    smtp: RawSmtp,
    #[serde(default)]
    secrets: Secrets,
    #[serde(default)]
    oauth: RawOAuth,
    #[serde(default)]
    icons: IconChoice,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Secrets {
    #[serde(default)]
    store: SecretStore,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, default)]
struct RawImap {
    host: String,
    port: u16,
    tls: Spanned<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, default)]
struct RawSmtp {
    host: String,
    port: u16,
    tls: Spanned<bool>,
}

impl Default for RawSmtp {
    fn default() -> Self {
        let gmail = Smtp::gmail();
        Self {
            host: gmail.host,
            port: gmail.port,
            tls: Spanned::new(0..0, gmail.tls),
        }
    }
}

/// Undocumented: only a test points Postal at a fake Google.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, default, rename_all = "kebab-case")]
struct RawOAuth {
    authorization_url: String,
    token_url: String,
    /// Seconds.
    redirect_timeout: u64,
}

impl Default for RawOAuth {
    fn default() -> Self {
        let google = Endpoints::google();
        Self {
            authorization_url: google.authorization_url,
            token_url: google.token_url,
            redirect_timeout: google.redirect_timeout.as_secs(),
        }
    }
}

impl Default for RawImap {
    fn default() -> Self {
        let gmail = Imap::gmail();
        Self {
            host: gmail.host,
            port: gmail.port,
            tls: Spanned::new(0..0, gmail.tls),
        }
    }
}

impl PartialConfig {
    pub fn load(path: &Path) -> Result<Self, Error> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(path, &text),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(Self {
                email: None,
                name: None,
                imap: Imap::gmail(),
                smtp: Smtp::gmail(),
                secrets: SecretStore::default(),
                oauth: Endpoints::google(),
                icons: IconChoice::default(),
            }),
            Err(source) => Err(Error::Read {
                path: path.to_owned(),
                source,
            }),
        }
    }

    fn parse(path: &Path, text: &str) -> Result<Self, Error> {
        let invalid = |span: Range<usize>, message: &str| {
            let (line, column) = line_and_column(text, span.start);
            Error::Invalid {
                path: path.to_owned(),
                line,
                column,
                message: message.trim_end().to_owned(),
            }
        };

        let raw: RawConfig =
            toml::from_str(text).map_err(|e| invalid(e.span().unwrap_or(0..0), e.message()))?;
        for (host, tls) in [
            (&raw.imap.host, &raw.imap.tls),
            (&raw.smtp.host, &raw.smtp.tls),
        ] {
            if !tls.get_ref() && !is_loopback(host) {
                return Err(invalid(
                    tls.span(),
                    "`tls = false` is allowed only for a loopback host",
                ));
            }
        }

        Ok(Self {
            email: raw.email,
            name: raw.name,
            imap: Imap {
                host: raw.imap.host,
                port: raw.imap.port,
                tls: *raw.imap.tls.get_ref(),
            },
            smtp: Smtp {
                host: raw.smtp.host,
                port: raw.smtp.port,
                tls: *raw.smtp.tls.get_ref(),
            },
            secrets: raw.secrets.store,
            oauth: Endpoints {
                authorization_url: raw.oauth.authorization_url,
                token_url: raw.oauth.token_url,
                redirect_timeout: Duration::from_secs(raw.oauth.redirect_timeout),
            },
            icons: raw.icons,
        })
    }

    /// The Config of this Account, whatever Account the file named.
    pub fn account(self, email: String) -> Config {
        Config {
            email,
            name: self.name,
            imap: self.imap,
            smtp: self.smtp,
            secrets: self.secrets,
            oauth: self.oauth,
            icons: self.icons,
        }
    }
}

fn is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// Both are 1-based; the column counts characters, not bytes.
fn line_and_column(text: &str, offset: usize) -> (usize, usize) {
    let before = text.get(..offset).unwrap_or(text);
    let line = before.rsplit('\n').next().unwrap_or_default();
    (before.matches('\n').count() + 1, line.chars().count() + 1)
}
