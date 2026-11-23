use std::{error::Error, fmt::Write, time::Duration};

use ring::digest::{SHA256, digest};

use crate::cli::version::Version;

/// The archive of this build's target, as `make dist` names it, and the
/// directory inside it that holds the binary.
pub const ARCHIVE: &str = concat!("postal-", env!("POSTAL_TARGET"));

const ARCHIVE_LIMIT: u64 = 256 * 1024 * 1024;

/// The latest release, read from where `latest` redirects: unlike GitHub's
/// API, the page has no rate limit.
pub fn latest() -> Result<Version, Box<dyn Error>> {
    let url = format!("{}/latest", base());
    let response = agent(Duration::from_secs(30), 0).get(&url).call()?;
    let tag = response
        .headers()
        .get("location")
        .and_then(|location| location.to_str().ok())
        .and_then(|location| location.rsplit_once("/tag/v"))
        .map(|(_, tag)| tag)
        .ok_or_else(|| format!("{url} names no release"))?;
    Ok(tag.parse()?)
}

/// This target's archive of release `version`, checked against its sha256.
pub fn archive(version: Version) -> Result<Vec<u8>, Box<dyn Error>> {
    let agent = agent(Duration::from_secs(300), 10);
    let url = format!("{}/download/v{version}/{ARCHIVE}.tar.gz", base());
    let archive = match agent.get(&url).call() {
        Err(ureq::Error::StatusCode(404)) => {
            return Err(format!("release {version} has no {ARCHIVE}; build from source").into());
        }
        response => response?
            .body_mut()
            .with_config()
            .limit(ARCHIVE_LIMIT)
            .read_to_vec()?,
    };

    let checksum = agent
        .get(&format!("{url}.sha256"))
        .call()?
        .body_mut()
        .read_to_string()?;
    let expected = checksum.split_whitespace().next().unwrap_or_default();
    if hex(digest(&SHA256, &archive).as_ref()) != expected.to_ascii_lowercase() {
        return Err(format!("{url} does not match its sha256").into());
    }

    Ok(archive)
}

/// Test seam: `POSTAL_RELEASES_URL` stands in for GitHub's releases page.
fn base() -> String {
    std::env::var("POSTAL_RELEASES_URL")
        .unwrap_or_else(|_unset| "https://github.com/kryptamine/postal/releases".to_owned())
}

fn agent(timeout: Duration, redirects: u32) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .max_redirects(redirects)
        .https_only(base().starts_with("https://"))
        .build()
        .into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        // Writing to a String cannot fail.
        write!(text, "{byte:02x}").ok();
        text
    })
}
