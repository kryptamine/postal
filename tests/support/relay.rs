//! The bytes between the client and the fake server. imap-codec knows no
//! Gmail extension, so `X-GM-MSGID` is taken out of each command here and
//! added to each `FETCH` answer, and commands are recorded as sent.

use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
};

const MSGID: &str = "X-GM-MSGID";

/// `record` gets each command line without its tag; `msgid` maps a UID to
/// its `X-GM-MSGID`, and is `None` for a server without Gmail's extensions.
pub async fn relay(
    client: TcpStream,
    server: TcpStream,
    record: impl Fn(String) + Send,
    msgid: Option<impl Fn(u32) -> Option<u64> + Send>,
) {
    let (client_read, client_write) = client.into_split();
    let (server_read, server_write) = server.into_split();
    let gmail = msgid.is_some();
    tokio::join!(
        commands(client_read, server_write, record, gmail),
        responses(server_read, client_write, msgid),
    );
}

async fn commands(
    from: OwnedReadHalf,
    mut to: OwnedWriteHalf,
    record: impl Fn(String),
    gmail: bool,
) {
    let mut reader = BufReader::new(from);
    let mut line = String::new();
    while reader.read_line(&mut line).await.is_ok_and(|n| n > 0) {
        let command = line.trim_end_matches(['\r', '\n']);
        let untagged = command.split_once(' ').map_or(command, |(_, rest)| rest);
        record(untagged.to_owned());
        let forwarded = if gmail {
            line.replace(&format!("{MSGID} "), "")
                .replace(&format!(" {MSGID}"), "")
        } else {
            line.clone()
        };

        if to.write_all(forwarded.as_bytes()).await.is_err() {
            break;
        }

        line.clear();
    }

    let _ = to.shutdown().await;
}

async fn responses(
    from: OwnedReadHalf,
    mut to: OwnedWriteHalf,
    msgid: Option<impl Fn(u32) -> Option<u64>>,
) {
    let mut reader = BufReader::new(from);
    let mut line = Vec::new();
    while reader
        .read_until(b'\n', &mut line)
        .await
        .is_ok_and(|n| n > 0)
    {
        let id = msgid
            .as_ref()
            .and_then(|msgid| fetched_uid(&line).and_then(msgid));
        if let Some(id) = id {
            let at = line.windows(7).position(|w| w == b"FETCH (").unwrap() + 7;
            line.splice(at..at, format!("{MSGID} {id} ").into_bytes());
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

    let _ = to.shutdown().await;
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
    let open = line.strip_suffix('}')?.rfind('{')?;
    line[open + 1..line.len() - 1].parse().ok()
}
