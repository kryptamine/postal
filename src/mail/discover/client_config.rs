//! A Thunderbird autoconfig document (`clientConfig`, version 1.1), fetched
//! over HTTP and read for the servers Postal can reach.

use roxmltree::{Document, Node};

use super::{Found, TIMEOUT};
use crate::{
    account::config::{Imap, Smtp},
    account::partial_config::is_loopback,
};

/// The servers the document at `url` names for `email`; none when it cannot
/// be fetched or read.
pub async fn fetch(url: String, email: &str) -> Found {
    let fetched = tokio::task::spawn_blocking(move || get(&url).map_err(|e| (url, e)))
        .await
        .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()));
    match fetched {
        Ok(xml) => parse(&xml, email),
        Err((url, error)) => {
            tracing::info!("no autoconfig at {url}: {error}");
            Found::default()
        }
    }
}

/// Blocks on the network.
fn get(url: &str) -> Result<String, ureq::Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        // An HTTPS document redirected to plain HTTP could name any host.
        .https_only(url.starts_with("https://"))
        .build()
        .into();
    agent.get(url).call()?.body_mut().read_to_string()
}

/// The first IMAP server over TLS and the first SMTP server over TLS or
/// STARTTLS; plaintext only to a loopback host, as the config allows.
fn parse(xml: &str, email: &str) -> Found {
    let document = match Document::parse(xml) {
        Ok(document) => document,
        Err(error) => {
            tracing::info!("an autoconfig document Postal cannot read: {error}");
            return Found::default();
        }
    };

    let servers = |tag: &'static str, kind: &'static str| {
        document
            .descendants()
            .filter(move |node| node.has_tag_name(tag) && node.attribute("type") == Some(kind))
    };

    Found {
        imap: servers("incomingServer", "imap").find_map(|node| imap(node, email)),
        smtp: servers("outgoingServer", "smtp").find_map(|node| smtp(node, email)),
    }
}

/// An endpoint of a server entry, before its security is chosen.
struct Entry {
    host: String,
    port: u16,
    socket: String,
    username: Option<String>,
}

fn entry(node: Node<'_, '_>, email: &str) -> Option<Entry> {
    let child = |name: &str| {
        node.children()
            .find(|child| child.has_tag_name(name))
            .and_then(|child| child.text())
            .map(str::trim)
    };

    let host = fill(child("hostname")?, email)?;
    let port = child("port")?.parse().ok().filter(|&port| port > 0)?;
    let socket = child("socketType")?.to_ascii_uppercase();
    // `%EMAILADDRESS%` is what a blank username stands for already.
    let username = child("username")
        .and_then(|username| fill(username, email))
        .filter(|username| !username.eq_ignore_ascii_case(email));
    Some(Entry {
        host,
        port,
        socket,
        username,
    })
}

/// Postal has no IMAP STARTTLS, so such an entry is passed over.
fn imap(node: Node<'_, '_>, email: &str) -> Option<Imap> {
    let entry = entry(node, email)?;
    let tls = match entry.socket.as_str() {
        "SSL" => true,
        "PLAIN" if is_loopback(&entry.host) => false,
        _ => return None,
    };

    Some(Imap {
        host: entry.host,
        port: entry.port,
        tls,
        username: entry.username,
    })
}

fn smtp(node: Node<'_, '_>, email: &str) -> Option<Smtp> {
    let entry = entry(node, email)?;
    let (tls, starttls) = match entry.socket.as_str() {
        "SSL" => (true, false),
        "STARTTLS" => (true, true),
        "PLAIN" if is_loopback(&entry.host) => (false, false),
        _ => return None,
    };

    Some(Smtp {
        host: entry.host,
        port: entry.port,
        tls,
        starttls,
        username: entry.username,
    })
}

/// `text` with the address's placeholders filled in; `None` when another
/// placeholder, say `%REALNAME%`, is left, or nothing is.
fn fill(text: &str, email: &str) -> Option<String> {
    let (local, domain) = email.rsplit_once('@').unwrap_or((email, ""));
    let filled = text
        .replace("%EMAILADDRESS%", email)
        .replace("%EMAILLOCALPART%", local)
        .replace("%EMAILDOMAIN%", domain);
    Some(filled).filter(|filled| !filled.is_empty() && !filled.contains('%'))
}
