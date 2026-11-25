use std::io;

use lettre::{Address, address::Envelope, message::Mailbox};

use crate::{
    account::config::Config, compose::draft::Draft, compose::unique_id::unique_id,
    domain::answering::Answering, render::signature::SEPARATOR,
};

/// The Account a Message goes out from.
#[derive(Clone, Copy)]
pub struct Outgoing<'a> {
    pub config: &'a Config,
}

impl Outgoing<'_> {
    /// `From` as Postal adds it: `name <email>`, or the bare address.
    pub fn from(self) -> Result<Mailbox, String> {
        Ok(Mailbox::new(self.config.name.clone(), self.address()?))
    }

    /// The signature under its separator, as a new Draft
    /// holds it; `None` for an Account that signs nothing.
    pub fn signature(self) -> io::Result<Option<String>> {
        let Some(signature) = &self.config.signature else {
            return Ok(None);
        };

        Ok(signature
            .text()?
            .map(|text| format!("{SEPARATOR}\n{text}\n")))
    }

    /// `draft` as it is sent and every retry sends it again, its `Date`
    /// and `Message-ID` fixed now.
    pub fn build(self, draft: &Draft, answering: Option<&Answering>) -> Result<Vec<u8>, String> {
        let format = self.config.compose_format;
        let message = draft.message(self.from()?, self.message_id(), answering, format)?;
        Ok(message.formatted())
    }

    /// Who `draft` goes to, as SMTP is told.
    pub fn envelope(self, draft: &Draft) -> Result<Envelope, String> {
        let to = draft.to.iter().chain(draft.cc.iter());
        let to = to.map(|mailbox| mailbox.email.clone()).collect();
        Envelope::new(Some(self.address()?), to).map_err(|error| error.to_string())
    }

    fn address(self) -> Result<Address, String> {
        let email = &self.config.email;
        email
            .parse()
            .map_err(|_bad| format!("the Account's address is not one: {email}"))
    }

    fn message_id(self) -> String {
        let email = &self.config.email;
        let domain = email
            .rsplit_once('@')
            .map_or("localhost", |(_, domain)| domain);
        format!("<{}@{domain}>", unique_id())
    }
}
