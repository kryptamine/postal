use futures::{FutureExt, future::LocalBoxFuture};
use lettre::message::Mailbox;

use crate::{config::Config, draft::Draft, sign_in::SignIn, smtp, unique_id::unique_id};

/// What a Send needs: the Account, and its sign-in while there is one.
#[derive(Clone, Copy)]
pub struct Outgoing<'a> {
    pub config: &'a Config,
    pub sign_in: Option<&'a SignIn>,
}

impl<'a> Outgoing<'a> {
    /// `From` as Postal adds it: `name <email>`, or the bare address.
    pub fn from(self) -> Result<Mailbox, String> {
        let email = &self.config.email;
        let address = email
            .parse()
            .map_err(|_bad| format!("the Account's address is not one: {email}"))?;
        Ok(Mailbox::new(self.config.name.clone(), address))
    }

    /// Builds `draft` with a fresh `Date` and `Message-ID` and sends it; the
    /// error is what to show in its preview.
    pub fn send(self, draft: &Draft) -> LocalBoxFuture<'a, Result<(), String>> {
        let built = self
            .sign_in
            .ok_or_else(|| "signed out: nothing was sent".to_owned())
            .and_then(|sign_in| {
                let message = draft.message(self.from()?, self.message_id())?;
                Ok((sign_in, message))
            });

        async move {
            let (sign_in, message) = built?;
            smtp::send(self.config, sign_in, message)
                .await
                .map_err(|error| error.to_string())
        }
        .boxed_local()
    }

    fn message_id(self) -> String {
        let email = &self.config.email;
        let domain = email
            .rsplit_once('@')
            .map_or("localhost", |(_, domain)| domain);
        format!("<{}@{domain}>", unique_id())
    }
}
