use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;

#[derive(Deserialize)]
struct Claims {
    email: String,
}

/// The Account's address from an `OpenID Connect` ID token. The signature goes
/// unchecked: the token came straight from Google's endpoint over TLS.
pub fn email(id_token: &str) -> Option<String> {
    let payload = id_token.split('.').nth(1)?;
    let json = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let claims: Claims = serde_json::from_slice(&json).ok()?;
    Some(claims.email)
}
