use std::{rc::Rc, time::Duration};

use futures::lock::Mutex;
use tokio::time::Instant;

use super::{
    Error,
    token_endpoint::{self, AccessToken},
};
use crate::account::credential::Credential;

/// An access token this close to its expiry is refreshed before use, so a
/// connection never starts with one about to lapse.
const MARGIN: Duration = Duration::from_secs(60);

/// The refresh token and the access token it last bought, kept in memory only.
pub struct Tokens {
    refresh: Credential,
    token_url: String,
    /// `None` until a refresh succeeds. Locked through a refresh, so
    /// connections opening together buy one access token between them.
    access: Mutex<Option<Access>>,
}

struct Access {
    token: Rc<Credential>,
    expires: Instant,
}

impl Tokens {
    pub fn new(refresh: Credential, token_url: String, access: AccessToken) -> Self {
        Self {
            refresh,
            token_url,
            access: Mutex::new(Some(Access::from(access))),
        }
    }

    /// The refresh token the secret store kept: the first connection buys
    /// the access token, so starting waits on no network.
    pub const fn stored(refresh: Credential, token_url: String) -> Self {
        Self {
            refresh,
            token_url,
            access: Mutex::new(None),
        }
    }

    pub const fn refresh_token(&self) -> &Credential {
        &self.refresh
    }

    /// The access token, refreshed first once it nears its expiry.
    #[expect(
        clippy::significant_drop_tightening,
        reason = "the lock is held through the refresh, so a second connection waits for its token"
    )]
    pub async fn access(&self) -> Result<Rc<Credential>, Error> {
        let mut access = self.access.lock().await;
        if let Some(access) = access.as_ref()
            && Instant::now() + MARGIN < access.expires
        {
            return Ok(Rc::clone(&access.token));
        }

        tracing::info!("refreshing the OAuth access token");
        let token_url = self.token_url.clone();
        let refresh = Credential::new(self.refresh.expose().to_owned());
        let fresh =
            tokio::task::spawn_blocking(move || token_endpoint::refresh(&token_url, &refresh))
                .await
                .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))?;
        let fresh = Access::from(fresh);
        let token = Rc::clone(&fresh.token);
        *access = Some(fresh);
        Ok(token)
    }
}

impl From<AccessToken> for Access {
    fn from(access: AccessToken) -> Self {
        Self {
            token: Rc::new(access.token),
            expires: Instant::now() + access.expires_in,
        }
    }
}
