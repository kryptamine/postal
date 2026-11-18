use super::{CLIENT_ID, MAIL_SCOPE, percent::encode, pkce::Pkce};

/// The page where the user grants Postal the Gmail scope. `prompt=consent`
/// makes Google issue a refresh token on every login, not only the first.
pub fn url(authorization_url: &str, redirect_uri: &str, pkce: &Pkce, state: &str) -> String {
    let scope = format!("{MAIL_SCOPE} openid email");
    let params = [
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", &scope),
        ("code_challenge", &pkce.challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("access_type", "offline"),
        ("prompt", "consent"),
    ];
    let query: Vec<String> = params
        .iter()
        .map(|(name, value)| format!("{name}={}", encode(value)))
        .collect();
    format!("{authorization_url}?{}", query.join("&"))
}
