use std::io::Write;

/// Turns the bytes of both directions into trace lines, with every literal
/// shown as `<N bytes>` and the client's LOGIN or AUTHENTICATE exchange dropped.
#[derive(Debug, Default)]
pub struct WireTrace {
    client: Lines,
    server: Lines,
    /// The tag of a login in flight: client lines stay out until it completes.
    login: Option<Vec<u8>>,
}

pub fn enabled() -> bool {
    tracing::enabled!(tracing::Level::TRACE)
}

impl WireTrace {
    pub fn sent(&mut self, bytes: &[u8]) {
        let login = &mut self.login;
        self.client.feed(bytes, |line| {
            let mut words = line.splitn(3, |&b| b == b' ');
            let tag = words.next().unwrap_or_default();
            let command = words.next().unwrap_or_default();
            if command.eq_ignore_ascii_case(b"LOGIN")
                || command.eq_ignore_ascii_case(b"AUTHENTICATE")
            {
                *login = Some(tag.to_owned());
            }

            if login.is_none() {
                tracing::trace!("C: {}", String::from_utf8_lossy(line));
            }
        });
    }

    pub fn received(&mut self, bytes: &[u8]) {
        let login = &mut self.login;
        self.server.feed(bytes, |line| {
            if let Some(tag) = login
                && line
                    .strip_prefix(tag.as_slice())
                    .is_some_and(|rest| rest.starts_with(b" "))
            {
                *login = None;
            }

            tracing::trace!("S: {}", String::from_utf8_lossy(line));
        });
    }
}

/// One direction of the stream, split into lines with literals elided.
#[derive(Debug, Default)]
struct Lines {
    line: Vec<u8>,
    /// Bytes of the current literal not yet received.
    skipping: usize,
}

impl Lines {
    fn feed(&mut self, mut bytes: &[u8], mut on_line: impl FnMut(&[u8])) {
        while !bytes.is_empty() {
            if self.skipping > 0 {
                let skipped = self.skipping.min(bytes.len());
                self.skipping -= skipped;
                bytes = &bytes[skipped..];
                continue;
            }

            let Some(end) = bytes.iter().position(|&b| b == b'\n') else {
                self.line.extend_from_slice(bytes);
                return;
            };

            self.line.extend_from_slice(&bytes[..end]);
            bytes = &bytes[end + 1..];
            if self.line.last() == Some(&b'\r') {
                self.line.pop();
            }

            if let Some((start, size)) = literal(&self.line) {
                self.line.truncate(start);
                let _ = write!(self.line, "<{size} bytes>");
                self.skipping = size;
            } else {
                on_line(&self.line);
                self.line.clear();
            }
        }
    }
}

/// Where a trailing `{N}`, `{N+}` or `~{N}` starts, and N.
fn literal(line: &[u8]) -> Option<(usize, usize)> {
    let open = line.strip_suffix(b"}")?.iter().rposition(|&b| b == b'{')?;
    let digits = &line[open + 1..line.len() - 1];
    let digits = digits.strip_suffix(b"+").unwrap_or(digits);
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }

    let size = std::str::from_utf8(digits).ok()?.parse().ok()?;
    let start = if open > 0 && line[open - 1] == b'~' {
        open - 1
    } else {
        open
    };
    Some((start, size))
}
