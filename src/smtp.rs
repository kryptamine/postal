use std::fmt;

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    address::Envelope,
    transport::smtp::{
        self,
        authentication::{Credentials, Mechanism},
        client::{Tls, TlsParameters},
    },
};

use crate::{
    config::Config,
    oauth::{self, Tokens},
};

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
/// Account by `XOAUTH2`.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn send(
    config: &Config,
    sign_in: &Tokens,
    (envelope, message): (&Envelope, &[u8]),
) -> Result<(), Error> {
    let access = sign_in.access().await.map_err(Error::Token)?;

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
        .credentials(Credentials::new(
            config.email.clone(),
            access.expose().to_owned(),
        ))
        .authentication(vec![Mechanism::Xoauth2])
        .build()
        .send_raw(envelope, message)
        .await
        .map_err(Error::Smtp)?;
    Ok(())
}
