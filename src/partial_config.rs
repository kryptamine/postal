use std::{io, net::IpAddr, ops::Range, path::Path, time::Duration};

use serde::Deserialize;
use toml::Spanned;

use crate::{
    config::{Config, Error, IconChoice, Imap, SecretStore, Smtp},
    oauth::Endpoints,
    provider::Provider,
};

/// `config.toml` as login reads it. The top level names the first Account
/// and sets what every Account leaves out; each `[[account]]` adds one. A
/// server set nowhere is the Provider's.
pub struct PartialConfig {
    accounts: Vec<RawAccount>,
    /// The display name of `From` every Account shares, unless it sets its own.
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
    #[serde(default)]
    account: Vec<RawAccount>,
}

/// One Account as written; what it leaves out is the top level's.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAccount {
    email: String,
    #[serde(default)]
    provider: Provider,
    name: Option<String>,
    #[serde(default)]
    imap: RawImap,
    #[serde(default)]
    smtp: RawSmtp,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Secrets {
    #[serde(default)]
    store: SecretStore,
}

#[derive(Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
struct RawImap {
    host: Option<String>,
    port: Option<u16>,
    tls: Option<Spanned<bool>>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
struct RawSmtp {
    host: Option<String>,
    port: Option<u16>,
    tls: Option<Spanned<bool>>,
    starttls: Option<bool>,
}

impl RawImap {
    /// This table over `base`, at `provider`.
    fn at(&self, base: &Self, provider: Provider) -> Imap {
        let default = Imap::of(provider);
        let tls = self.tls.as_ref().or(base.tls.as_ref());
        Imap {
            host: self
                .host
                .clone()
                .or_else(|| base.host.clone())
                .unwrap_or(default.host),
            port: self.port.or(base.port).unwrap_or(default.port),
            tls: tls.map_or(default.tls, |tls| *tls.get_ref()),
        }
    }
}

impl RawSmtp {
    fn at(&self, base: &Self, provider: Provider) -> Smtp {
        let default = Smtp::of(provider);
        let tls = self.tls.as_ref().or(base.tls.as_ref());
        Smtp {
            host: self
                .host
                .clone()
                .or_else(|| base.host.clone())
                .unwrap_or(default.host),
            port: self.port.or(base.port).unwrap_or(default.port),
            tls: tls.map_or(default.tls, |tls| *tls.get_ref()),
            starttls: self.starttls.or(base.starttls).unwrap_or(default.starttls),
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
                accounts: Vec::new(),
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
        let invalid =
            |span: Range<usize>, message: &str| Error::invalid(path, text, span.start, message);

        let raw: RawConfig =
            toml::from_str(text).map_err(|e| invalid(e.span().unwrap_or(0..0), e.message()))?;
        let tables = std::iter::once((&raw.imap, &raw.smtp)).chain(
            raw.account
                .iter()
                .map(|account| (&account.imap, &account.smtp)),
        );
        for (imap, smtp) in tables {
            // A host left out is a Provider's, never on loopback.
            for (host, tls) in [(&imap.host, &imap.tls), (&smtp.host, &smtp.tls)] {
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
        }

        let first = raw.email.map(|email| RawAccount {
            email,
            provider: raw.provider,
            name: None,
            imap: RawImap::default(),
            smtp: RawSmtp::default(),
        });

        Ok(Self {
            accounts: first.into_iter().chain(raw.account).collect(),
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

    /// Every Account named, in the order written.
    pub fn configs(&self) -> Vec<Config> {
        self.accounts
            .iter()
            .map(|account| self.config(account, account.email.clone(), account.provider))
            .collect()
    }

    /// The Config of this Account at `provider`, with what the file sets
    /// for this address, if anything.
    pub fn account(&self, email: String, provider: Provider) -> Config {
        let fallback = RawAccount {
            email: String::new(),
            provider,
            name: None,
            imap: RawImap::default(),
            smtp: RawSmtp::default(),
        };

        let written = self.find(&email).map_or(&fallback, |at| &self.accounts[at]);
        self.config(written, email, provider)
    }

    /// The Provider this address is written with, if it is.
    pub fn provider_of(&self, email: &str) -> Option<Provider> {
        self.find(email).map(|at| self.accounts[at].provider)
    }

    /// Every Account named, in the order written.
    pub fn accounts(&self) -> impl Iterator<Item = (&str, Provider)> {
        self.accounts
            .iter()
            .map(|account| (account.email.as_str(), account.provider))
    }

    fn find(&self, email: &str) -> Option<usize> {
        self.accounts
            .iter()
            .position(|account| account.email.eq_ignore_ascii_case(email))
    }

    fn config(&self, account: &RawAccount, email: String, provider: Provider) -> Config {
        Config {
            email,
            provider,
            name: account.name.clone().or_else(|| self.name.clone()),
            imap: account.imap.at(&self.imap, provider),
            smtp: account.smtp.at(&self.smtp, provider),
            secrets: self.secrets,
            oauth: self.oauth.clone(),
            icons: self.icons,
        }
    }
}

fn is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}
