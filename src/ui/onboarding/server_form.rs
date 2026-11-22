use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    account::config::{Imap, Smtp},
    account::credential::Credential,
    account::partial_config::is_loopback,
    account::provider::Provider,
    mail::login::Servers,
};

/// How a connection is encrypted. `None` is offered for a loopback host
/// alone, as the config allows it nowhere else.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Security {
    Tls,
    StartTls,
    None,
}

impl Security {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Tls => "TLS",
            Self::StartTls => "STARTTLS",
            Self::None => "none",
        }
    }
}

/// One row of the form, in the order shown.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    ImapHost,
    ImapPort,
    ImapSecurity,
    ImapUsername,
    SmtpHost,
    SmtpPort,
    SmtpSecurity,
    SmtpUsername,
    SmtpPassword,
}

pub const SLOTS: [Slot; 9] = [
    Slot::ImapHost,
    Slot::ImapPort,
    Slot::ImapSecurity,
    Slot::ImapUsername,
    Slot::SmtpHost,
    Slot::SmtpPort,
    Slot::SmtpSecurity,
    Slot::SmtpUsername,
    Slot::SmtpPassword,
];

impl Slot {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ImapHost => "IMAP server",
            Self::ImapPort => "IMAP port",
            Self::ImapSecurity => "IMAP security",
            Self::ImapUsername => "IMAP username",
            Self::SmtpHost => "SMTP server",
            Self::SmtpPort => "SMTP port",
            Self::SmtpSecurity => "SMTP security",
            Self::SmtpUsername => "SMTP username",
            Self::SmtpPassword => "SMTP password",
        }
    }

    /// What an empty field stands for.
    pub const fn blank(self) -> &'static str {
        match self {
            Self::ImapUsername => "the address",
            Self::SmtpUsername => "as IMAP",
            Self::SmtpPassword => "as IMAP, or as before",
            _ => "",
        }
    }
}

pub enum Shown<'a> {
    Text(&'a str),
    /// A password, this many characters long.
    Dots(usize),
    Choice(Security),
}

pub enum Keyed {
    Stay,
    Submit,
    Back,
    Cancel,
}

/// The servers of a generic Provider as typed, from a guess by the domain
/// or the Account's config.
pub struct ServerForm {
    /// The address the form was filled for.
    email: String,
    imap_host: String,
    imap_port: String,
    imap_security: Security,
    imap_username: String,
    smtp_host: String,
    smtp_port: String,
    smtp_security: Security,
    smtp_username: String,
    smtp_password: String,
    pub focus: usize,
    /// Why the last `Enter` was refused.
    pub problem: Option<&'static str>,
}

impl ServerForm {
    pub fn guess(email: &str) -> Self {
        let (imap, smtp) = (
            Imap::of(Provider::Other, email),
            Smtp::of(Provider::Other, email),
        );
        Self::of(email, &imap, &smtp)
    }

    pub fn of(email: &str, imap: &Imap, smtp: &Smtp) -> Self {
        let smtp_security = match (smtp.tls, smtp.starttls) {
            (false, _) => Security::None,
            (true, true) => Security::StartTls,
            (true, false) => Security::Tls,
        };

        Self {
            email: email.to_owned(),
            imap_host: imap.host.clone(),
            imap_port: imap.port.to_string(),
            imap_security: if imap.tls {
                Security::Tls
            } else {
                Security::None
            },
            imap_username: imap.username.clone().unwrap_or_default(),
            smtp_host: smtp.host.clone(),
            smtp_port: smtp.port.to_string(),
            smtp_security,
            smtp_username: smtp.username.clone().unwrap_or_default(),
            smtp_password: String::new(),
            focus: 0,
            problem: None,
        }
    }

    /// Whether it was filled for `email`, so a guess need not be made again.
    pub fn is_for(&self, email: &str) -> bool {
        self.email.eq_ignore_ascii_case(email)
    }

    pub fn shown(&self, slot: Slot) -> Shown<'_> {
        match slot {
            Slot::ImapHost => Shown::Text(&self.imap_host),
            Slot::ImapPort => Shown::Text(&self.imap_port),
            Slot::ImapSecurity => Shown::Choice(self.imap_security),
            Slot::ImapUsername => Shown::Text(&self.imap_username),
            Slot::SmtpHost => Shown::Text(&self.smtp_host),
            Slot::SmtpPort => Shown::Text(&self.smtp_port),
            Slot::SmtpSecurity => Shown::Choice(self.smtp_security),
            Slot::SmtpUsername => Shown::Text(&self.smtp_username),
            Slot::SmtpPassword => Shown::Dots(self.smtp_password.chars().count()),
        }
    }

    /// Applies `event`: `Tab` and the arrows move, `←→` and `Space` change a
    /// choice, every printable key types into a text field.
    pub fn key(&mut self, event: KeyEvent) -> Keyed {
        let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
        let slot = SLOTS[self.focus];
        let choice = matches!(slot, Slot::ImapSecurity | Slot::SmtpSecurity);
        match event.code {
            KeyCode::Char('c') if ctrl => return Keyed::Cancel,
            KeyCode::Char('u') if ctrl => {
                if let Some(text) = self.text_mut(slot) {
                    text.clear();
                }
            }
            _ if ctrl => {}
            KeyCode::Tab | KeyCode::Down => self.focus = (self.focus + 1) % SLOTS.len(),
            KeyCode::BackTab | KeyCode::Up => {
                self.focus = (self.focus + SLOTS.len() - 1) % SLOTS.len();
            }
            KeyCode::Right | KeyCode::Char(' ') if choice => self.cycle(slot, true),
            KeyCode::Left if choice => self.cycle(slot, false),
            KeyCode::Char(c) => {
                let port = matches!(slot, Slot::ImapPort | Slot::SmtpPort);
                if let Some(text) = self.text_mut(slot)
                    && (!port || (c.is_ascii_digit() && text.len() < 5))
                {
                    text.push(c);
                }
            }
            KeyCode::Backspace => {
                if let Some(text) = self.text_mut(slot) {
                    text.pop();
                }
            }
            KeyCode::Enter => {
                self.problem = self.servers().err();
                if self.problem.is_none() {
                    return Keyed::Submit;
                }
            }
            KeyCode::Esc => return Keyed::Back,
            _ => {}
        }

        Keyed::Stay
    }

    const fn text_mut(&mut self, slot: Slot) -> Option<&mut String> {
        match slot {
            Slot::ImapHost => Some(&mut self.imap_host),
            Slot::ImapPort => Some(&mut self.imap_port),
            Slot::ImapUsername => Some(&mut self.imap_username),
            Slot::SmtpHost => Some(&mut self.smtp_host),
            Slot::SmtpPort => Some(&mut self.smtp_port),
            Slot::SmtpUsername => Some(&mut self.smtp_username),
            Slot::SmtpPassword => Some(&mut self.smtp_password),
            Slot::ImapSecurity | Slot::SmtpSecurity => None,
        }
    }

    /// The next choice, or the one before; SMTP's port follows TLS and
    /// STARTTLS while it is the other's usual one.
    fn cycle(&mut self, slot: Slot, forward: bool) {
        let (host, security, choices): (_, _, &[Security]) = match slot {
            Slot::ImapSecurity => (
                &self.imap_host,
                self.imap_security,
                &[Security::Tls, Security::None],
            ),
            _ => (
                &self.smtp_host,
                self.smtp_security,
                &[Security::Tls, Security::StartTls, Security::None],
            ),
        };

        let offered: Vec<_> = choices
            .iter()
            .copied()
            .filter(|&s| s != Security::None || is_loopback(host.trim()))
            .collect();
        let at = offered.iter().position(|&s| s == security).unwrap_or(0);
        let step = if forward { 1 } else { offered.len() - 1 };
        let next = offered[(at + step) % offered.len()];
        if slot == Slot::ImapSecurity {
            self.imap_security = next;
            return;
        }

        self.smtp_security = next;
        self.smtp_port = match (next, self.smtp_port.as_str()) {
            (Security::StartTls, "465") => "587".to_owned(),
            (Security::Tls, "587") => "465".to_owned(),
            _ => std::mem::take(&mut self.smtp_port),
        };
    }

    /// The servers as typed, or what is wrong with them.
    pub fn servers(&self) -> Result<Servers, &'static str> {
        let host = |host: &str| {
            let host = host.trim();
            if host.is_empty() || host.contains(char::is_whitespace) {
                return Err("a server is a host name, with no spaces");
            }

            Ok(host.to_owned())
        };

        let port = |port: &str| match port.parse::<u16>() {
            Ok(port) if port > 0 => Ok(port),
            _ => Err("a port is a number from 1 to 65535"),
        };

        let named = |text: &str| Some(text.trim().to_owned()).filter(|text| !text.is_empty());
        let (imap_host, smtp_host) = (host(&self.imap_host)?, host(&self.smtp_host)?);
        let unencrypted = [
            (self.imap_security, &imap_host),
            (self.smtp_security, &smtp_host),
        ];
        if unencrypted
            .iter()
            .any(|(security, host)| *security == Security::None && !is_loopback(host))
        {
            return Err("only a loopback host is reached without TLS");
        }

        Ok(Servers {
            imap: Imap {
                host: imap_host,
                port: port(&self.imap_port)?,
                tls: self.imap_security != Security::None,
                username: named(&self.imap_username),
            },
            smtp: Smtp {
                host: smtp_host,
                port: port(&self.smtp_port)?,
                tls: self.smtp_security != Security::None,
                starttls: self.smtp_security == Security::StartTls,
                username: named(&self.smtp_username),
            },
            smtp_password: named(&self.smtp_password).map(Credential::new),
        })
    }
}
