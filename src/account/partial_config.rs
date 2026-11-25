use std::{
    io,
    net::IpAddr,
    ops::Range,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Deserialize;
use toml::Spanned;

use crate::{
    account::autoconfig::Autoconfig,
    account::config::{
        ComposeFormat, Config, ContactsChoice, Error, IconChoice, Imap, NotifyChoice, SEND_DELAY,
        SecretStore, Smtp, Terminal,
    },
    account::oauth::Endpoints,
    account::provider::Provider,
    account::signature::Signature,
};

/// `config.toml` as login reads it. The top level names the first Account
/// and sets what every Account leaves out; each `[[account]]` adds one. A
/// server set nowhere is the Provider's.
pub struct PartialConfig {
    /// Where `config.toml` is, which a relative `signature-file` is beside.
    dir: PathBuf,
    accounts: Vec<RawAccount>,
    /// The display name of `From` every Account shares, unless it sets its own.
    pub name: Option<String>,
    imap: RawImap,
    smtp: RawSmtp,
    pub secrets: SecretStore,
    pub oauth: Endpoints,
    pub autoconfig: Autoconfig,
    pub icons: IconChoice,
    pub notify: NotifyChoice,
    pub terminal: Terminal,
    pub contacts: ContactsChoice,
    /// The format of every Account that sets none of its own.
    pub compose_format: ComposeFormat,
    /// How long a Send is held before it leaves, for `u` to take it back.
    pub send_delay: Duration,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    email: Option<String>,
    #[serde(default)]
    provider: Provider,
    name: Option<String>,
    signature: Option<String>,
    #[serde(rename = "signature-file")]
    signature_file: Option<Spanned<String>>,
    #[serde(default)]
    imap: RawImap,
    #[serde(default)]
    smtp: RawSmtp,
    #[serde(default)]
    secrets: Secrets,
    #[serde(default)]
    oauth: RawOAuth,
    #[serde(default)]
    autoconfig: Autoconfig,
    #[serde(default)]
    icons: IconChoice,
    #[serde(default)]
    notify: NotifyChoice,
    #[serde(default)]
    terminal: Terminal,
    #[serde(default)]
    contacts: ContactsChoice,
    #[serde(default, rename = "compose-format")]
    compose_format: ComposeFormat,
    /// In whole seconds.
    #[serde(rename = "send-delay")]
    send_delay: Option<u64>,
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
    signature: Option<String>,
    #[serde(rename = "signature-file")]
    signature_file: Option<Spanned<String>>,
    #[serde(rename = "compose-format")]
    compose_format: Option<ComposeFormat>,
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
    username: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
struct RawSmtp {
    host: Option<String>,
    port: Option<u16>,
    tls: Option<Spanned<bool>>,
    starttls: Option<bool>,
    username: Option<String>,
}

impl RawImap {
    /// This table over `base`, at `provider`, for `email`.
    fn at(&self, base: &Self, provider: Provider, email: &str) -> Imap {
        let default = Imap::of(provider, email);
        let tls = self.tls.as_ref().or(base.tls.as_ref());
        Imap {
            host: self
                .host
                .clone()
                .or_else(|| base.host.clone())
                .unwrap_or(default.host),
            port: self.port.or(base.port).unwrap_or(default.port),
            tls: tls.map_or(default.tls, |tls| *tls.get_ref()),
            username: self.username.clone().or_else(|| base.username.clone()),
        }
    }
}

impl RawSmtp {
    fn at(&self, base: &Self, provider: Provider, email: &str) -> Smtp {
        let default = Smtp::of(provider, email);
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
            username: self.username.clone().or_else(|| base.username.clone()),
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
                dir: PathBuf::new(),
                accounts: Vec::new(),
                name: None,
                imap: RawImap::default(),
                smtp: RawSmtp::default(),
                secrets: SecretStore::default(),
                oauth: Endpoints::google(),
                autoconfig: Autoconfig::default(),
                icons: IconChoice::default(),
                notify: NotifyChoice::default(),
                terminal: Terminal::default(),
                contacts: ContactsChoice::default(),
                compose_format: ComposeFormat::default(),
                send_delay: SEND_DELAY,
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

        if let Some(twice) = signed_twice(&raw) {
            let message = "`signature` and `signature-file` are both set: keep one";
            return Err(invalid(twice, message));
        }

        let dir = path.parent().unwrap_or(path);
        // The top level signs for the Account it names, and no other.
        let first = raw.email.map(|email| RawAccount {
            email,
            provider: raw.provider,
            name: None,
            signature: raw.signature,
            signature_file: raw.signature_file,
            compose_format: None,
            imap: RawImap::default(),
            smtp: RawSmtp::default(),
        });

        Ok(Self {
            dir: dir.to_owned(),
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
            autoconfig: raw.autoconfig,
            icons: raw.icons,
            notify: raw.notify,
            terminal: raw.terminal,
            contacts: raw.contacts,
            compose_format: raw.compose_format,
            send_delay: raw.send_delay.map_or(SEND_DELAY, Duration::from_secs),
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
            signature: None,
            signature_file: None,
            compose_format: None,
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

    /// The signature of the Account at this address, if it is given one.
    pub fn signature_of(&self, email: &str) -> Option<Signature> {
        self.signature(&self.accounts[self.find(email)?])
    }

    fn signature(&self, account: &RawAccount) -> Option<Signature> {
        let file = account.signature_file.as_ref();
        let file = file.map(|file| Signature::file(&self.dir, file.get_ref()));
        file.or_else(|| account.signature.clone().map(Signature::Text))
    }

    fn find(&self, email: &str) -> Option<usize> {
        self.accounts
            .iter()
            .position(|account| account.email.eq_ignore_ascii_case(email))
    }

    fn config(&self, account: &RawAccount, email: String, provider: Provider) -> Config {
        Config {
            imap: account.imap.at(&self.imap, provider, &email),
            smtp: account.smtp.at(&self.smtp, provider, &email),
            email,
            provider,
            name: account.name.clone().or_else(|| self.name.clone()),
            signature: self.signature(account),
            compose_format: account.compose_format.unwrap_or(self.compose_format),
            secrets: self.secrets,
            oauth: self.oauth.clone(),
        }
    }
}

/// Where a table sets its signature a second time, as a file.
fn signed_twice(raw: &RawConfig) -> Option<Range<usize>> {
    let accounts = raw.account.iter();
    let tables = std::iter::once((&raw.signature, &raw.signature_file))
        .chain(accounts.map(|account| (&account.signature, &account.signature_file)));
    tables
        .filter(|(text, _)| text.is_some())
        .find_map(|(_, file)| file.as_ref().map(Spanned::span))
}

pub fn is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}
