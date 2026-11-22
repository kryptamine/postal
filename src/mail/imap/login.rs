use async_imap::{Client, Session};

use super::{Error, connect::Transport, xoauth2::XOAuth2};
use crate::{account::config::Config, account::provider::Provider, account::sign_in::SignIn};

/// Signs in as the Account: by `XOAUTH2`, or by `LOGIN` with the username
/// the config names, else the whole address. iCloud tries the name part
/// first: it wants that for its own domains, the address for a custom one.
pub async fn sign_in(
    client: Client<Box<dyn Transport>>,
    config: &Config,
    sign_in: &SignIn,
) -> Result<Session<Box<dyn Transport>>, Error> {
    let password = match sign_in {
        SignIn::OAuth(tokens) => {
            let token = tokens.access().await?;
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
        SignIn::Password { password, .. } => password.expose(),
    };

    let email = config.email.as_str();
    if let Some(username) = &config.imap.username {
        return Ok(client.login(username, password).await.map_err(|(e, _)| e)?);
    }

    let name = email.split_once('@').map(|(name, _)| name);
    let Some(name) = name.filter(|_| config.provider == Provider::ICloud) else {
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
