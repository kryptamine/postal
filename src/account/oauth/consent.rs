use super::{CLIENT_ID, MAIL_SCOPE, percent::encode, pkce::Pkce};

/// The page where the user grants Postal the Gmail scope, with the email
/// `hint` filled in. `prompt=consent` makes Google issue a refresh token on
/// every login, not only the first.
pub fn url(
    authorization_url: &str,
    redirect_uri: &str,
    pkce: &Pkce,
    state: &str,
    hint: Option<&str>,
) -> String {
    let scope = format!("{MAIL_SCOPE} openid email");
    let mut params = vec![
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
    params.extend(hint.map(|hint| ("login_hint", hint)));
    let query: Vec<String> = params
        .iter()
        .map(|(name, value)| format!("{name}={}", encode(value)))
        .collect();
    format!("{authorization_url}?{}", query.join("&"))
}
