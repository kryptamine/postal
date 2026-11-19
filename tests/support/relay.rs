//! The bytes between the client and the fake server. imap-codec knows no
//! Gmail extension, so `X-GM-MSGID` and `X-GM-THRID` are taken out of each
//! command here, a search by `X-GM-THRID` passed on as a header search, and
//! both added to each `FETCH` answer. Commands are recorded as sent.

use std::sync::{Arc, Mutex};

use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
};

const MSGID: &str = "X-GM-MSGID";
const THRID: &str = "X-GM-THRID";

/// `record` gets each command line without its tag; `ids` maps a Folder and
/// UID to its `X-GM-MSGID` and `X-GM-THRID`, and is `None` for a server
/// without Gmail's extensions.
pub async fn relay(
    client: TcpStream,
    server: TcpStream,
    record: impl Fn(String) + Send,
    ids: Option<impl Fn(&str, u32) -> Option<(u64, u64)> + Send>,
) {
    let (client_read, client_write) = client.into_split();
    let (server_read, server_write) = server.into_split();
    let gmail = ids.is_some();
    let examined = Arc::new(Mutex::new(String::from("INBOX")));
    tokio::join!(
        commands(client_read, server_write, (record, &examined), gmail),
        responses(server_read, client_write, (ids, &examined)),
    );
}

async fn commands(
    from: OwnedReadHalf,
    mut to: OwnedWriteHalf,
    (record, examined): (impl Fn(String), &Mutex<String>),
    gmail: bool,
) {
    let mut reader = BufReader::new(from);
    let mut line = String::new();
    while reader.read_line(&mut line).await.is_ok_and(|n| n > 0) {
        let command = line.trim_end_matches(['\r', '\n']);
        let untagged = command.split_once(' ').map_or(command, |(_, rest)| rest);
        if let Some(name) = untagged
            .strip_prefix("EXAMINE ")
            .or_else(|| untagged.strip_prefix("SELECT "))
        {
            *examined.lock().unwrap() = unquoted(name);
        }

        record(untagged.to_owned());
        let thread_search = format!("UID SEARCH {THRID} ");
        let forwarded = if gmail && untagged.starts_with(&thread_search) {
            // Passed on as a header search the fake knows how to answer.
            line.replacen(&thread_search, &format!("UID SEARCH HEADER {THRID} "), 1)
        } else if gmail {
            [MSGID, THRID].iter().fold(line.clone(), |line, item| {
                line.replace(&format!("{item} "), "")
                    .replace(&format!(" {item}"), "")
            })
        } else {
            line.clone()
        };

        if to.write_all(forwarded.as_bytes()).await.is_err()
            || !pass_literals(&mut reader, &mut to, literal_len(line.as_bytes())).await
        {
            break;
        }

        line.clear();
    }

    to.shutdown().await.ok();
}

/// Passes on a command's literal of `len` bytes, as an `APPEND` sends, and
/// the rest of its line, none of it recorded; false once either side is gone.
async fn pass_literals(
    reader: &mut BufReader<OwnedReadHalf>,
    to: &mut OwnedWriteHalf,
    mut len: Option<usize>,
) -> bool {
    while let Some(n) = len {
        let mut bytes = vec![0; n];
        let mut rest = String::new();
        if reader.read_exact(&mut bytes).await.is_err()
            || reader.read_line(&mut rest).await.is_err()
            || to.write_all(&bytes).await.is_err()
            || to.write_all(rest.as_bytes()).await.is_err()
        {
            return false;
        }

        len = literal_len(rest.as_bytes());
    }

    true
}

async fn responses(
    from: OwnedReadHalf,
    mut to: OwnedWriteHalf,
    (ids, examined): (
        Option<impl Fn(&str, u32) -> Option<(u64, u64)>>,
        &Mutex<String>,
    ),
) {
    let mut reader = BufReader::new(from);
    let mut line = Vec::new();
    while reader
        .read_until(b'\n', &mut line)
        .await
        .is_ok_and(|n| n > 0)
    {
        let found = ids
            .as_ref()
            .and_then(|ids| ids(&examined.lock().unwrap(), fetched_uid(&line)?));
        if let Some((msgid, thrid)) = found {
            let at = line.windows(7).position(|w| w == b"FETCH (").unwrap() + 7;
            line.splice(
                at..at,
                format!("{MSGID} {msgid} {THRID} {thrid} ").into_bytes(),
            );
        }

        let literal = literal_len(&line);
        if to.write_all(&line).await.is_err() {
            break;
        }

        // A literal's bytes are passed on as they are, never read as lines.
        if let Some(len) = literal {
            let mut bytes = vec![0; len];
            if reader.read_exact(&mut bytes).await.is_err() || to.write_all(&bytes).await.is_err() {
                break;
            }
        }

        line.clear();
    }

    to.shutdown().await.ok();
}

/// The UID of a `* n FETCH (UID u ...` line, as the fake writes it.
fn fetched_uid(line: &[u8]) -> Option<u32> {
    let line = std::str::from_utf8(line).ok()?;
    let (_, rest) = line.strip_prefix("* ")?.split_once(" FETCH (UID ")?;
    let digits = rest.split(|c: char| !c.is_ascii_digit()).next()?;
    digits.parse().ok()
}

/// N, for a line ending in the literal marker `{N}`.
fn literal_len(line: &[u8]) -> Option<usize> {
    let line = std::str::from_utf8(line)
        .ok()?
        .trim_end_matches(['\r', '\n']);
    let (_, n) = line.strip_suffix('}')?.rsplit_once('{')?;
    n.parse().ok()
}

/// A mailbox name as async-imap quotes it.
fn unquoted(name: &str) -> String {
    let inner = name
        .strip_prefix('"')
        .and_then(|n| n.strip_suffix('"'))
        .unwrap_or(name);
    inner.replace("\\\"", "\"").replace("\\\\", "\\")
}
