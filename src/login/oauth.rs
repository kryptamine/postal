use std::path::Path;

use super::{Error, save, verify, writes};
use crate::{
    browser,
    credential::Credential,
    oauth::{
        Endpoints, Tokens, consent,
        loopback::{Loopback, Outcome},
        pkce::{self, Pkce},
        token_endpoint,
    },
    partial_config::PartialConfig,
};

/// The Onboarding through Google's consent page, which offers the Account
/// `hint` first: the Account is whoever signs in there. Nothing is written
/// before the IMAP server accepts it.
pub fn oauth(dir: &Path, hint: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let partial = PartialConfig::load(&dir.join("config.toml"))?;
    let endpoints = partial.oauth.clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let pkce = Pkce::new()?;
    let state = pkce::random()?;
    let (code, redirect_uri) =
        runtime.block_on(wait_for_consent(&endpoints, &pkce, &state, hint))?;
    let grant =
        token_endpoint::exchange(&endpoints.token_url, &code, &pkce.verifier, &redirect_uri)?;
    if !grant.includes_mail() {
        return Err(Error::NoMailScope.into());
    }

    let writes = writes(&partial, &grant.email)?;
    let config = partial.account(grant.email);
    let sign_in = Tokens::new(grant.refresh, endpoints.token_url, grant.access);
    runtime.block_on(verify(&config, &sign_in))?;
    save(dir, &config, &sign_in, writes)
}

/// Sends the user to the consent page and waits for the browser to come back;
/// returns the code with the redirect URI it came to.
#[expect(
    clippy::print_stdout,
    reason = "sign-in runs before the TUI and talks on stdout"
)]
async fn wait_for_consent(
    endpoints: &Endpoints,
    pkce: &Pkce,
    state: &str,
    hint: Option<&str>,
) -> Result<(Credential, String), Error> {
    let loopback = Loopback::bind().await.map_err(Error::Loopback)?;
    let url = consent::url(
        &endpoints.authorization_url,
        &loopback.redirect_uri,
        pkce,
        state,
        hint,
    );
    println!("Sign in to Google in the browser. If it did not open, visit:\n{url}");
    if let Err(error) = browser::open(&url) {
        tracing::warn!("cannot open the browser: {error}");
    }

    let waited = endpoints.redirect_timeout;
    let outcome = tokio::select! {
        outcome = tokio::time::timeout(waited, loopback.outcome(state)) => outcome,
        // Without raw mode Ctrl-C is SIGINT, which would end the TUI too.
        _ = tokio::signal::ctrl_c() => return Err(Error::Cancelled),
    };

    let outcome = outcome
        .map_err(|_elapsed| Error::NoRedirect(waited))?
        .map_err(Error::Loopback)?;
    match outcome {
        Outcome::Code(code) => Ok((code, loopback.redirect_uri)),
        Outcome::Refused(code) => Err(Error::Denied(code)),
    }
}
