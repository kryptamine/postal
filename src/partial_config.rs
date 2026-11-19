use std::{io, net::IpAddr, ops::Range, path::Path, time::Duration};

use serde::Deserialize;
use toml::Spanned;

use crate::{
    config::{Config, Error, IconChoice, Imap, SecretStore, Smtp},
    oauth::Endpoints,
    provider::Provider,
};

/// `config.toml` as login reads it: everything but the Account may be set
/// already, and a server not set is the Provider's.
pub struct PartialConfig {
    pub email: Option<String>,
    pub provider: Provider,
    pub name: Option<String>,
    imap: RawImap,
    smtp: RawSmtp,
    pub secrets: SecretStore,
    pub oauth: Endpoints,
    pub icons: IconChoice,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    email: Option<String>,
    #[serde(default)]
    provider: Provider,
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

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawImap {
    host: Option<String>,
    port: Option<u16>,
    tls: Option<Spanned<bool>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawSmtp {
    host: Option<String>,
    port: Option<u16>,
    tls: Option<Spanned<bool>>,
    starttls: Option<bool>,
}

impl RawImap {
    fn at(self, provider: Provider) -> Imap {
        let default = Imap::of(provider);
        Imap {
            host: self.host.unwrap_or(default.host),
            port: self.port.unwrap_or(default.port),
            tls: self.tls.map_or(default.tls, Spanned::into_inner),
        }
    }
}

impl RawSmtp {
    fn at(self, provider: Provider) -> Smtp {
        let default = Smtp::of(provider);
        Smtp {
            host: self.host.unwrap_or(default.host),
            port: self.port.unwrap_or(default.port),
            tls: self.tls.map_or(default.tls, Spanned::into_inner),
            starttls: self.starttls.unwrap_or(default.starttls),
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

impl PartialConfig {
    pub fn load(path: &Path) -> Result<Self, Error> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(path, &text),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(Self {
                email: None,
                provider: Provider::default(),
                name: None,
                imap: RawImap::default(),
                smtp: RawSmtp::default(),
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
        // A host left out is a Provider's, never on loopback.
        for (host, tls) in [
            (&raw.imap.host, &raw.imap.tls),
            (&raw.smtp.host, &raw.smtp.tls),
        ] {
            if let Some(tls) = tls
                && !tls.get_ref()
                && !host.as_deref().is_some_and(is_loopback)
            {
                return Err(invalid(
                    tls.span(),
                    "`tls = false` is allowed only for a loopback host",
                ));
            }
        }

        Ok(Self {
            email: raw.email,
            provider: raw.provider,
            name: raw.name,
            imap: raw.imap,
            smtp: raw.smtp,
            secrets: raw.secrets.store,
            oauth: Endpoints {
                authorization_url: raw.oauth.authorization_url,
                token_url: raw.oauth.token_url,
                redirect_timeout: Duration::from_secs(raw.oauth.redirect_timeout),
            },
            icons: raw.icons,
        })
    }

    /// The Config of this Account at `provider`, whatever Account the file
    /// named; a server the file does not set is the Provider's.
    pub fn account(self, email: String, provider: Provider) -> Config {
        Config {
            email,
            provider,
            name: self.name,
            imap: self.imap.at(provider),
            smtp: self.smtp.at(provider),
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
