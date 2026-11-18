//! Signing in through Google's OAuth: the consent in the browser with PKCE,
//! and the token endpoint. See docs/adr/0003-embedded-oauth-client.md.

pub mod consent;
mod endpoints;
mod error;
mod id_token;
pub mod loopback;
mod percent;
pub mod pkce;
pub mod token_endpoint;
mod tokens;

pub use endpoints::Endpoints;
pub use error::Error;
pub use tokens::Tokens;

const CLIENT_ID: &str = "442919188815-gkqj554vrqismnb0hgbouofsl31ad29n.apps.googleusercontent.com";
// Google treats an installed app's client secret as public.
const CLIENT_SECRET: &str = "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";

/// The scope IMAP needs; Google's consent screen lets the user untick it.
pub const MAIL_SCOPE: &str = "https://mail.google.com/";
