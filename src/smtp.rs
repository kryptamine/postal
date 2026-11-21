use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    address::Envelope,
    transport::smtp::{
        self,
        authentication::{Credentials, Mechanism},
        client::{Tls, TlsParameters},
    },
};

use crate::{config::Config, oauth, sign_in::SignIn};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The OAuth access token could not be refreshed.
    #[error(transparent)]
    Token(oauth::Error),
    /// The connection failed, or the server refused: its answer, as it said it.
    #[error("SMTP: {0}")]
    Smtp(smtp::Error),
}

impl Error {
    /// A `5xx` reply: sending the same Message again fails the same way.
    /// A `53x` refuses the sign-in, which is the Account's to fix.
    pub fn refused(&self) -> bool {
        let Self::Smtp(source) = self else {
            return false;
        };

        let sign_in = source
            .status()
            .is_some_and(|code| code.to_string().starts_with("53"));
        source.is_permanent() && !sign_in
    }
}

/// Hands `message` to the `[smtp]` server for `envelope`, signed in as the
/// Account: by `XOAUTH2`, or by its password and full address.
pub async fn send(
    config: &Config,
    sign_in: &SignIn,
    (envelope, message): (&Envelope, &[u8]),
) -> Result<(), Error> {
    let (secret, mechanisms) = match sign_in {
        SignIn::OAuth(tokens) => {
            let access = tokens.access().await.map_err(Error::Token)?;
            (access.expose().to_owned(), vec![Mechanism::Xoauth2])
        }
        SignIn::Password(password) => (
            password.expose().to_owned(),
            vec![Mechanism::Plain, Mechanism::Login],
        ),
    };

    let server = &config.smtp;
    // Plaintext is allowed only on loopback, by the config.
    let tls = if !server.tls {
        Tls::None
    } else if server.starttls {
        Tls::Required(TlsParameters::new(server.host.clone()).map_err(Error::Smtp)?)
    } else {
        Tls::Wrapper(TlsParameters::new(server.host.clone()).map_err(Error::Smtp)?)
    };

    AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&server.host)
        .port(server.port)
        .tls(tls)
        .credentials(Credentials::new(config.email.clone(), secret))
        .authentication(mechanisms)
        .build()
        .send_raw(envelope, message)
        .await
        .map_err(Error::Smtp)?;
    Ok(())
}
