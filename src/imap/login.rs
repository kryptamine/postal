use async_imap::{Client, Session};

use super::{Error, connect::Transport, xoauth2::XOAuth2};
use crate::{config::Config, sign_in::SignIn};

/// Signs in as the Account: by `XOAUTH2`, or by `LOGIN` with the name part of
/// the address, then the whole address once that is refused, as iCloud wants
/// the one for its own domains and the other for a custom domain.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn sign_in(
    client: Client<Box<dyn Transport>>,
    config: &Config,
    sign_in: &SignIn,
) -> Result<Session<Box<dyn Transport>>, Error> {
    let password = match sign_in {
        SignIn::OAuth(tokens) => {
            let token = tokens.access().await.map_err(Error::Token)?;
            let xoauth2 = XOAuth2 {
                email: &config.email,
                token: token.expose(),
                sent: false,
            };

            return Ok(client
                .authenticate("XOAUTH2", xoauth2)
                .await
                .map_err(|(e, _)| e)?);
        }
        SignIn::Password(password) => password.expose(),
    };

    let email = config.email.as_str();
    let Some((name, _)) = email.split_once('@') else {
        return Ok(client.login(email, password).await.map_err(|(e, _)| e)?);
    };

    match client.login(name, password).await {
        Ok(session) => Ok(session),
        Err((error, client)) => {
            let error = Error::from(error);
            if !error.bad_credentials() {
                return Err(error);
            }

            tracing::info!("LOGIN as the name part refused: trying the whole address");
            Ok(client.login(email, password).await.map_err(|(e, _)| e)?)
        }
    }
}
