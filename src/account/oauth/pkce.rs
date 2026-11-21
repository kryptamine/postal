use std::io;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};

/// RFC 7636: the verifier stays with Postal, the challenge goes in the
/// consent URL.
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn new() -> io::Result<Self> {
        let verifier = random()?;
        let challenge =
            URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()));
        Ok(Self {
            verifier,
            challenge,
        })
    }
}

/// 32 random bytes, base64url: a 43-character verifier, or a `state`.
pub fn random() -> io::Result<String> {
    let mut bytes = [0; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_unspecified| io::Error::other("no system randomness"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
