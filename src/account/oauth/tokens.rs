use std::{cell::RefCell, rc::Rc, time::Duration};

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
    /// `None` until a refresh succeeds.
    access: RefCell<Option<Access>>,
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
            access: RefCell::new(Some(Access::from(access))),
        }
    }

    /// Trades `refresh` for a first access token, blocking on the network.
    /// Offline, the first connection tries again.
    pub fn refreshed(refresh: Credential, token_url: String) -> Result<Self, Error> {
        let access = match token_endpoint::refresh(&token_url, &refresh) {
            Ok(access) => Some(Access::from(access)),
            Err(Error::Http(reason)) => {
                tracing::warn!("cannot refresh the access token at start: {reason}");
                None
            }
            Err(error) => return Err(error),
        };

        Ok(Self {
            refresh,
            token_url,
            access: RefCell::new(access),
        })
    }

    pub const fn refresh_token(&self) -> &Credential {
        &self.refresh
    }

    /// The access token, refreshed first once it nears its expiry.
    pub async fn access(&self) -> Result<Rc<Credential>, Error> {
        {
            let access = self.access.borrow();
            if let Some(access) = access.as_ref()
                && Instant::now() + MARGIN < access.expires
            {
                return Ok(Rc::clone(&access.token));
            }
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
        *self.access.borrow_mut() = Some(fresh);
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
