//! A generic Provider's servers as the domain publishes them: its own
//! autoconfig document, then Thunderbird's ISPDB, then DNS SRV (RFC 6186).
//! A source that fails or times out is logged and passed over.

mod client_config;
mod dns_message;
mod srv;

use std::time::Duration;

use crate::{
    account::autoconfig::Autoconfig,
    account::config::{Imap, Smtp, domain},
};

/// For each HTTP request and each DNS query; they all run at once.
const TIMEOUT: Duration = Duration::from_secs(4);

/// What was found for each server; `None` where every source was silent.
#[derive(Default)]
pub struct Found {
    pub imap: Option<Imap>,
    pub smtp: Option<Smtp>,
}

impl Found {
    /// This, with `other`'s server where this has none.
    fn or(self, other: Self) -> Self {
        Self {
            imap: self.imap.or(other.imap),
            smtp: self.smtp.or(other.smtp),
        }
    }
}

/// The servers published for the domain of `email`. The address itself is
/// sent nowhere: each source is asked by the domain alone.
pub async fn discover(email: &str, sources: &Autoconfig) -> Found {
    let domain = domain(email);
    let plausible = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '.';
    if domain.is_empty() || !domain.chars().all(plausible) {
        tracing::info!("no server lookup for the domain {domain:?}");
        return Found::default();
    }

    #[expect(
        clippy::literal_string_with_formatting_args,
        reason = "`{domain}` is the placeholder of a URL in the config"
    )]
    let url = |template: &str| template.replace("{domain}", &domain);
    let (own, ispdb, srv) = tokio::join!(
        client_config::fetch(url(&sources.domain_url), email),
        client_config::fetch(url(&sources.ispdb_url), email),
        srv::lookup(&domain, sources.dns_server),
    );

    let found = own.or(ispdb).or(srv);
    if found.imap.is_none() && found.smtp.is_none() {
        tracing::info!("no servers published for {domain}: guessing from the domain");
    }

    found
}
