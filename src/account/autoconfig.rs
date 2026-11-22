use std::net::SocketAddr;

use serde::Deserialize;

/// Where a generic Provider's servers are looked up; `{domain}` in a URL is
/// the address's. Only a test overrides them, through the undocumented
/// `[autoconfig]` table, to reach fakes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, default, rename_all = "kebab-case")]
pub struct Autoconfig {
    /// The domain's own autoconfig document.
    pub domain_url: String,
    /// Thunderbird's ISPDB.
    pub ispdb_url: String,
    /// The DNS server asked for SRV records; else `/etc/resolv.conf`'s.
    pub dns_server: Option<SocketAddr>,
}

impl Default for Autoconfig {
    fn default() -> Self {
        Self {
            domain_url: "https://autoconfig.{domain}/mail/config-v1.1.xml".to_owned(),
            ispdb_url: "https://autoconfig.thunderbird.net/v1.1/{domain}".to_owned(),
            dns_server: None,
        }
    }
}
