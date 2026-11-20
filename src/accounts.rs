use crate::{config::Config, outgoing::Outgoing};

/// The Accounts of this run, each known by its place in it.
#[derive(Clone, Copy)]
pub struct Accounts<'a>(pub &'a [Config]);

impl<'a> Accounts<'a> {
    pub fn email(self, account: usize) -> &'a str {
        self.0.get(account).map_or("", |config| &config.email)
    }

    /// The address a Draft's notes keep: none with one Account, which
    /// every Draft goes out from.
    pub fn noted(self, account: usize) -> Option<&'a str> {
        Some(self.email(account)).filter(|_| self.0.len() > 1)
    }

    /// The place of the Account a Draft's notes name; the first's for none
    /// or one no longer in the config.
    pub fn place(self, email: Option<&str>) -> usize {
        email
            .and_then(|email| {
                self.0
                    .iter()
                    .position(|config| config.email.eq_ignore_ascii_case(email))
            })
            .unwrap_or(0)
    }

    pub fn outgoing(self, account: usize) -> Option<Outgoing<'a>> {
        self.0.get(account).map(|config| Outgoing { config })
    }
}
