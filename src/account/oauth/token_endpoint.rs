use std::time::Duration;

use serde::Deserialize;

use super::{CLIENT_ID, CLIENT_SECRET, Error, MAIL_SCOPE, id_token};
use crate::account::credential::Credential;

const TIMEOUT: Duration = Duration::from_secs(30);

pub struct AccessToken {
    pub token: Credential,
    pub expires_in: Duration,
}

impl AccessToken {
    const fn new(token: String, expires_in: u64) -> Self {
        Self {
            token: Credential::new(token),
            expires_in: Duration::from_secs(expires_in),
        }
    }
}

/// What the authorization code is traded for.
pub struct Grant {
    pub access: AccessToken,
    pub refresh: Credential,
    /// Space-separated, as granted rather than as asked.
    scope: String,
    pub email: String,
}

impl Grant {
    pub fn includes_mail(&self) -> bool {
        self.scope.split(' ').any(|scope| scope == MAIL_SCOPE)
    }
}

#[derive(Deserialize)]
struct Answer {
    access_token: String,
    expires_in: u64,
    refresh_token: Option<String>,
    #[serde(default)]
    scope: String,
    id_token: Option<String>,
}

#[derive(Deserialize)]
struct Refusal {
    error: String,
    error_description: Option<String>,
}

/// Blocks on the network.
pub fn exchange(
    token_url: &str,
    code: &Credential,
    verifier: &str,
    redirect_uri: &str,
) -> Result<Grant, Error> {
    let answer = post(
        token_url,
        &[
            ("grant_type", "authorization_code"),
            ("code", code.expose()),
            ("code_verifier", verifier),
            ("redirect_uri", redirect_uri),
        ],
    )?;
    let missing = |field| Error::Invalid(format!("no `{field}`"));
    let refresh = answer
        .refresh_token
        .ok_or_else(|| missing("refresh_token"))?;
    let email = answer
        .id_token
        .as_deref()
        .and_then(id_token::email)
        .ok_or_else(|| missing("email in `id_token`"))?;
    Ok(Grant {
        access: AccessToken::new(answer.access_token, answer.expires_in),
        refresh: Credential::new(refresh),
        scope: answer.scope,
        email,
    })
}

/// Blocks on the network.
pub fn refresh(token_url: &str, refresh: &Credential) -> Result<AccessToken, Error> {
    let answer = post(
        token_url,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh.expose()),
        ],
    )?;
    Ok(AccessToken::new(answer.access_token, answer.expires_in))
}

fn post(token_url: &str, form: &[(&str, &str)]) -> Result<Answer, Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();
    let http = |e: ureq::Error| Error::Http(e.to_string());
    let client = [("client_id", CLIENT_ID), ("client_secret", CLIENT_SECRET)];
    let mut response = agent
        .post(token_url)
        .send_form(client.iter().chain(form).copied())
        .map_err(http)?;
    let success = response.status().is_success();
    let body = response.body_mut().read_to_string().map_err(http)?;
    if success {
        return serde_json::from_str(&body).map_err(|e| Error::Invalid(e.to_string()));
    }

    let refusal: Refusal = serde_json::from_str(&body)
        .map_err(|_unparsed| Error::Invalid(format!("HTTP {}", response.status())))?;
    Err(Error::Refused {
        error: refusal.error,
        description: refusal.error_description,
    })
}
