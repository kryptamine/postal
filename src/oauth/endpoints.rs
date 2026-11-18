use std::time::Duration;

/// Where the OAuth flow talks to Google. Only a test overrides them, through
/// the undocumented `[oauth]` table, to reach a fake.
#[derive(Clone, PartialEq, Eq)]
pub struct Endpoints {
    pub authorization_url: String,
    pub token_url: String,
    /// How long login waits for the browser to come back.
    pub redirect_timeout: Duration,
}

impl Endpoints {
    pub fn google() -> Self {
        Self {
            authorization_url: "https://accounts.google.com/o/oauth2/v2/auth".to_owned(),
            token_url: "https://oauth2.googleapis.com/token".to_owned(),
            redirect_timeout: Duration::from_secs(5 * 60),
        }
    }
}
