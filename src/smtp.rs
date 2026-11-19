use std::fmt;

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    transport::smtp::{
        self,
        authentication::{Credentials, Mechanism},
        client::{Tls, TlsParameters},
    },
};

use crate::{config::Config, oauth, sign_in::SignIn};

#[derive(Debug)]
pub enum Error {
    /// The OAuth access token could not be refreshed.
    Token(oauth::Error),
    /// The connection failed, or the server refused: its answer, as it said it.
    Smtp(smtp::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Token(source) => source.fmt(f),
            Self::Smtp(source) => write!(f, "SMTP: {source}"),
        }
    }
}

impl std::error::Error for Error {}

/// Hands `message` to the `[smtp]` server, signed in as the Account: by
/// `XOAUTH2` with an access token, or by `PLAIN` with an app password.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn send(config: &Config, sign_in: &SignIn, message: Message) -> Result<(), Error> {
    let (secret, mechanism) = match sign_in {
        SignIn::Password(password) => (password.expose().to_owned(), Mechanism::Plain),
        SignIn::OAuth(tokens) => {
            let access = tokens.access().await.map_err(Error::Token)?;
            (access.expose().to_owned(), Mechanism::Xoauth2)
        }
    };

    let server = &config.smtp;
    // Implicit TLS, or plaintext, which the config allows only on loopback.
    let tls = if server.tls {
        Tls::Wrapper(TlsParameters::new(server.host.clone()).map_err(Error::Smtp)?)
    } else {
        Tls::None
    };

    AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&server.host)
        .port(server.port)
        .tls(tls)
        .credentials(Credentials::new(config.email.clone(), secret))
        .authentication(vec![mechanism])
        .build()
        .send(message)
        .await
        .map_err(Error::Smtp)?;
    Ok(())
}
