use std::{io, net::Ipv4Addr};

use futures::{StreamExt, stream::FuturesUnordered};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use super::percent;
use crate::credential::Credential;

/// Enough for the request line and headers of any browser.
const HEAD_LIMIT: usize = 16 * 1024;

/// What the consent page sent the browser back with.
pub enum Outcome {
    Code(Credential),
    /// Google's `error` code, such as `access_denied`.
    Refused(String),
}

/// The listener on `127.0.0.1` that the consent page redirects to.
pub struct Loopback {
    listener: TcpListener,
    pub redirect_uri: String,
}

impl Loopback {
    pub async fn bind() -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let redirect_uri = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
        Ok(Self {
            listener,
            redirect_uri,
        })
    }

    /// Waits for the redirect carrying `state`. Any other request, such as a
    /// favicon or a forged redirect, is answered 404 and waited past. Requests
    /// are read side by side: a browser may open a socket and never send on it.
    pub async fn outcome(&self, state: &str) -> io::Result<Outcome> {
        let mut reading = FuturesUnordered::new();
        loop {
            let (mut stream, head) = tokio::select! {
                accepted = self.listener.accept() => {
                    reading.push(read_head(accepted?.0));
                    continue;
                }
                Some(read) = reading.next() => read,
            };

            let Some(outcome) = head.ok().and_then(|head| outcome(&head, state)) else {
                respond(&mut stream, "404 Not Found", "Not found.").await;
                continue;
            };

            let page = match outcome {
                Outcome::Code(_) => "Postal got the sign-in. Return to the terminal.",
                Outcome::Refused(_) => "The sign-in did not go through. Return to the terminal.",
            };

            respond(&mut stream, "200 OK", page).await;
            return Ok(outcome);
        }
    }
}

async fn read_head(mut stream: TcpStream) -> (TcpStream, io::Result<String>) {
    let mut head = Vec::new();
    let mut buffer = [0; 1024];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        let read = match stream.read(&mut buffer).await {
            Ok(read) if read > 0 && head.len() + read <= HEAD_LIMIT => read,
            Ok(_) => return (stream, Err(io::ErrorKind::InvalidData.into())),
            Err(error) => return (stream, Err(error)),
        };

        head.extend_from_slice(&buffer[..read]);
    }

    let head = String::from_utf8(head).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
    (stream, head)
}

/// `None` unless `head` is a `GET` whose query carries `state` and either a
/// `code` or an `error`.
fn outcome(head: &str, state: &str) -> Option<Outcome> {
    let target = head
        .lines()
        .next()?
        .strip_prefix("GET ")?
        .split(' ')
        .next()?;
    let (_, query) = target.split_once('?')?;
    let (mut code, mut error, mut matched) = (None, None, false);
    for pair in query.split('&') {
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };

        let value = percent::decode(value)?;
        match name {
            "state" => matched = value == state,
            "code" => code = Some(value),
            "error" => error = Some(value),
            _ => {}
        }
    }

    if !matched {
        return None;
    }

    match (code, error) {
        (_, Some(error)) => Some(Outcome::Refused(error)),
        (Some(code), None) => Some(Outcome::Code(Credential::new(code))),
        (None, None) => None,
    }
}

/// The browser shows `page`; a failure to send it changes nothing.
async fn respond(stream: &mut TcpStream, status: &str, page: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
        page.len()
    );
    stream.write_all(response.as_bytes()).await.ok();
    stream.shutdown().await.ok();
}
