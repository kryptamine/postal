use std::{
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Deserialize;

pub use crate::account::secret_store::SecretStore;
use crate::{
    account::oauth::Endpoints, account::partial_config::PartialConfig, account::provider::Provider,
    account::signature::Signature,
};

/// One Account's settings from `config.toml`, with those every Account shares.
pub struct Config {
    pub email: String,
    pub provider: Provider,
    /// The display name of `From`; without it, the bare address.
    pub name: Option<String>,
    pub signature: Option<Signature>,
    pub imap: Imap,
    pub smtp: Smtp,
    pub secrets: SecretStore,
    pub oauth: Endpoints,
}

/// `send-delay` when `config.toml` sets none.
pub const SEND_DELAY: Duration = Duration::from_secs(10);

#[derive(PartialEq, Eq)]
pub struct Imap {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    /// What `LOGIN` names in place of the address.
    pub username: Option<String>,
}

impl Imap {
    /// The Provider's server; for `Other`, guessed from the domain of `email`.
    pub fn of(provider: Provider, email: &str) -> Self {
        let host = match provider {
            Provider::Google => "imap.gmail.com".to_owned(),
            Provider::ICloud => "imap.mail.me.com".to_owned(),
            Provider::Other => format!("imap.{}", domain(email)),
        };

        Self {
            host,
            port: 993,
            tls: true,
            username: None,
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
    /// What `AUTH` names, else the IMAP username, else the address.
    pub username: Option<String>,
}

impl Smtp {
    /// iCloud's SMTP takes STARTTLS on 587 alone (support.apple.com/102525).
    pub fn of(provider: Provider, email: &str) -> Self {
        let (host, port, starttls) = match provider {
            Provider::Google => ("smtp.gmail.com".to_owned(), 465, false),
            Provider::ICloud => ("smtp.mail.me.com".to_owned(), 587, true),
            Provider::Other => (format!("smtp.{}", domain(email)), 465, false),
        };

        Self {
            host,
            port,
            tls: true,
            starttls,
            username: None,
        }
    }
}

/// What follows the last `@`, without case.
pub fn domain(email: &str) -> String {
    let domain = email.rsplit_once('@').map_or(email, |(_, domain)| domain);
    domain.trim().to_lowercase()
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
}

/// When new mail in an Inbox raises a desktop notification.
#[derive(Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NotifyChoice {
    /// Only while the terminal does not have focus.
    #[default]
    Unfocused,
    Always,
    Never,
}

impl NotifyChoice {
    /// The value of `notify` in `config.toml`.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Unfocused => "unfocused",
            Self::Always => "always",
            Self::Never => "never",
        }
    }
}

/// Where Correspondents come from besides mail: `macos` adds the people
/// in the system's Contacts, read anew each time (ADR 0035).
#[derive(Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ContactsChoice {
    #[default]
    None,
    Macos,
}

impl ContactsChoice {
    /// The value of `contacts` in `config.toml`.
    pub const fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Macos => "macos",
        }
    }
}

/// What a mailto: link opens Postal in on macOS: `terminal = "ghostty"`,
/// or `terminal = { command = "..." }` for a program of the user's own.
#[derive(Deserialize, Clone, PartialEq, Eq)]
#[serde(from = "WrittenTerminal")]
pub enum Terminal {
    App(TerminalApp),
    /// Run with Postal's path and the link.
    Command(String),
}

impl Default for Terminal {
    fn default() -> Self {
        Self::App(TerminalApp::default())
    }
}

#[derive(Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalApp {
    /// Terminal.app.
    #[default]
    Terminal,
    Ghostty,
    Kitty,
    Wezterm,
    Iterm2,
}

impl TerminalApp {
    pub const ALL: [Self; 5] = [
        Self::Terminal,
        Self::Ghostty,
        Self::Kitty,
        Self::Wezterm,
        Self::Iterm2,
    ];

    /// The value of `terminal` in `config.toml`.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Ghostty => "ghostty",
            Self::Kitty => "kitty",
            Self::Wezterm => "wezterm",
            Self::Iterm2 => "iterm2",
        }
    }
}

/// `terminal` as written: a name, or a table naming a command.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WrittenTerminal {
    Terminal,
    Ghostty,
    Kitty,
    Wezterm,
    Iterm2,
    Command(String),
}

impl From<WrittenTerminal> for Terminal {
    fn from(written: WrittenTerminal) -> Self {
        let app = match written {
            WrittenTerminal::Command(command) => return Self::Command(command),
            WrittenTerminal::Terminal => TerminalApp::Terminal,
            WrittenTerminal::Ghostty => TerminalApp::Ghostty,
            WrittenTerminal::Kitty => TerminalApp::Kitty,
            WrittenTerminal::Wezterm => TerminalApp::Wezterm,
            WrittenTerminal::Iterm2 => TerminalApp::Iterm2,
        };

        Self::App(app)
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
