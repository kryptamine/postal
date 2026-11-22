use std::{
    cmp::Reverse,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

use ring::rand::{SecureRandom, SystemRandom};
use tokio::net::UdpSocket;

use super::{
    Found, TIMEOUT,
    dns_message::{self, Srv},
};
use crate::account::config::{Imap, Smtp};

/// The servers the SRV records of `domain` name (RFC 6186), asked of
/// `server`, else of the system's resolver. `_imap._tcp` is not asked:
/// Postal has no IMAP STARTTLS.
pub async fn lookup(domain: &str, server: Option<SocketAddr>) -> Found {
    let Some(server) = server.or_else(system_resolver) else {
        tracing::info!("no DNS server in /etc/resolv.conf for SRV records");
        return Found::default();
    };

    let (imaps, submissions, submission) = tokio::join!(
        best(server, "_imaps._tcp", domain),
        best(server, "_submissions._tcp", domain),
        best(server, "_submission._tcp", domain),
    );

    let smtp = |srv: Srv, starttls| Smtp {
        host: srv.target,
        port: srv.port,
        tls: true,
        starttls,
        username: None,
    };

    Found {
        imap: imaps.map(|srv| Imap {
            host: srv.target,
            port: srv.port,
            tls: true,
            username: None,
        }),
        smtp: submissions
            .map(|srv| smtp(srv, false))
            .or_else(|| submission.map(|srv| smtp(srv, true))),
    }
}

/// The record of `service` to use: the lowest priority, then the heaviest.
/// A target of `.` says the domain offers no such service.
async fn best(server: SocketAddr, service: &str, domain: &str) -> Option<Srv> {
    let name = format!("{service}.{domain}");
    let records = match tokio::time::timeout(TIMEOUT, ask(server, &name)).await {
        Ok(Ok(records)) => records,
        Ok(Err(error)) => {
            tracing::info!("no SRV records of {name}: {error}");
            return None;
        }
        Err(_elapsed) => {
            tracing::info!("no SRV records of {name}: {server} did not answer");
            return None;
        }
    };

    let best = records
        .into_iter()
        .min_by_key(|srv| (srv.priority, Reverse(srv.weight)))?;
    Some(best).filter(|srv| !srv.target.is_empty())
}

async fn ask(server: SocketAddr, name: &str) -> Result<Vec<Srv>, Box<dyn std::error::Error>> {
    let mut id = [0; 2];
    SystemRandom::new()
        .fill(&mut id)
        .map_err(|_unspecified| "no random number for a DNS query")?;
    let id = u16::from_be_bytes(id);
    let any: IpAddr = if server.is_ipv4() {
        Ipv4Addr::UNSPECIFIED.into()
    } else {
        Ipv6Addr::UNSPECIFIED.into()
    };

    let socket = UdpSocket::bind((any, 0)).await?;
    socket.connect(server).await?;
    socket.send(&dns_message::query(id, name)?).await?;
    let mut buffer = [0; 4096];
    loop {
        let length = socket.recv(&mut buffer).await?;
        // An answer to another query, say a late one, is passed over.
        if let Some(records) = dns_message::answer(id, &buffer[..length])? {
            return Ok(records);
        }
    }
}

/// The first `nameserver` of `/etc/resolv.conf`.
fn system_resolver() -> Option<SocketAddr> {
    let text = std::fs::read_to_string("/etc/resolv.conf").ok()?;
    text.lines().find_map(|line| {
        let address = line.trim().strip_prefix("nameserver")?.trim();
        // An IPv6 address may carry a `%interface` scope, which `IpAddr` refuses.
        let address = address.split('%').next()?;
        Some(SocketAddr::new(address.parse().ok()?, 53))
    })
}
