use std::path::Path;

use super::{Error, Granted, verify, writes};
use crate::{
    browser,
    credential::Credential,
    oauth::{
        Tokens, consent,
        loopback::{Loopback, Outcome},
        pkce::{self, Pkce},
        token_endpoint,
    },
    partial_config::PartialConfig,
    provider::Provider,
    sign_in::SignIn,
};

/// Google's consent page, opened in the browser: the Account is whoever
/// signs in there. Nothing is written before the IMAP server accepts it.
pub struct Consent {
    partial: PartialConfig,
    loopback: Loopback,
    pkce: Pkce,
    state: String,
    /// For the user to open when the browser did not.
    pub url: String,
}

impl Consent {
    /// Opens the page, which offers the Account `hint` first.
    pub async fn open(dir: &Path, hint: Option<&str>) -> Result<Self, Box<dyn std::error::Error>> {
        let partial = PartialConfig::load(&dir.join("config.toml"))?;
        let pkce = Pkce::new()?;
        let state = pkce::random()?;
        let loopback = Loopback::bind().await.map_err(Error::Loopback)?;
        let url = consent::url(
            &partial.oauth.authorization_url,
            &loopback.redirect_uri,
            &pkce,
            &state,
            hint,
        );
        if let Err(error) = browser::open(&url) {
            tracing::warn!("cannot open the browser: {error}");
        }

        Ok(Self {
            partial,
            loopback,
            pkce,
            state,
            url,
        })
    }

    /// The code the browser came back with.
    pub async fn code(&self) -> Result<Credential, Error> {
        let waited = self.partial.oauth.redirect_timeout;
        let outcome = tokio::time::timeout(waited, self.loopback.outcome(&self.state))
            .await
            .map_err(|_elapsed| Error::NoRedirect(waited))?
            .map_err(Error::Loopback)?;
        match outcome {
            Outcome::Code(code) => Ok(code),
            Outcome::Refused(code) => Err(Error::Denied(code)),
        }
    }

    /// Trades `code` for tokens and signs in to the IMAP server with them.
    #[expect(
        clippy::future_not_send,
        reason = "Postal runs on a current-thread runtime"
    )]
    pub async fn grant(self, code: &Credential) -> Result<Granted, Box<dyn std::error::Error>> {
        let token_url = self.partial.oauth.token_url.clone();
        let grant = token_endpoint::exchange(
            &token_url,
            code,
            &self.pkce.verifier,
            &self.loopback.redirect_uri,
        )?;
        if !grant.includes_mail() {
            return Err(Error::NoMailScope.into());
        }

        let writes = writes(&self.partial, &grant.email, Provider::Google);
        let config = self.partial.account(grant.email, Provider::Google);
        let sign_in = SignIn::OAuth(Tokens::new(grant.refresh, token_url, grant.access));
        verify(&config, &sign_in).await?;
        Ok(Granted {
            partial: self.partial,
            config,
            sign_in,
            writes,
        })
    }
}
