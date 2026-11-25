use crate::{
    account::config::{ComposeFormat, Config},
    compose::outgoing::Outgoing,
    domain::per_account::{AccountId, PerAccount},
};

/// The Accounts of this run.
#[derive(Clone, Copy)]
pub struct Accounts<'a>(pub &'a PerAccount<Config>);

impl<'a> Accounts<'a> {
    pub fn email(self, account: AccountId) -> &'a str {
        &self.0[account].email
    }

    /// The address a Draft's notes keep: none with one Account, which
    /// every Draft goes out from.
    pub fn noted(self, account: AccountId) -> Option<&'a str> {
        (self.0.len() > 1).then(|| self.email(account))
    }

    /// The Account a Draft's notes name; the first for none or one no
    /// longer in the config.
    pub fn place(self, email: Option<&str>) -> Option<AccountId> {
        email
            .and_then(|email| {
                self.0
                    .position(|config| config.email.eq_ignore_ascii_case(email))
            })
            .or_else(|| self.0.first())
    }

    /// The format a Draft whose notes name `email` goes out in, as Send
    /// places it.
    pub fn format(self, email: Option<&str>) -> ComposeFormat {
        self.place(email)
            .map_or_else(ComposeFormat::default, |account| {
                self.0[account].compose_format
            })
    }

    pub fn outgoing(self, account: AccountId) -> Outgoing<'a> {
        Outgoing {
            config: &self.0[account],
        }
    }
}
