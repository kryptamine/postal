/// The content of part `section` (as in `BODY[1.2]`) of `message`, a whole
/// Message as `BODY[]` sends it, still in its transfer encoding; `None`
/// when the Message has no such part. Attached Messages are not entered.
pub fn section<'a>(message: &'a [u8], section: &str) -> Option<&'a [u8]> {
    let mut entity = message;
    for number in section.split('.') {
        let index = number.parse::<usize>().ok()?.checked_sub(1)?;
        let (headers, body) = split(entity);
        match boundary(headers) {
            Some(boundary) => entity = parts(body, &boundary).into_iter().nth(index)?,
            // A part that is not multipart is its own part 1.
            None if index == 0 => {}
            None => return None,
        }
    }

    Some(split(entity).1)
}

/// The header block and the content after the blank line that ends it.
fn split(entity: &[u8]) -> (&[u8], &[u8]) {
    if let Some(content) = entity
        .strip_prefix(b"\r\n")
        .or_else(|| entity.strip_prefix(b"\n"))
    {
        return (&[], content);
    }

    let crlf = find(entity, b"\r\n\r\n").map(|at| (at, 4));
    let lf = find(entity, b"\n\n").map(|at| (at, 2));
    let end = match (crlf, lf) {
        (Some(crlf), Some(lf)) => Some(if lf.0 < crlf.0 { lf } else { crlf }),
        (crlf, lf) => crlf.or(lf),
    };

    end.map_or((entity, &[]), |(at, len)| {
        (&entity[..at], &entity[at + len..])
    })
}

/// The `boundary` parameter of a `multipart/*` `Content-Type`.
fn boundary(headers: &[u8]) -> Option<String> {
    let headers = String::from_utf8_lossy(headers);
    let unfolded = headers
        .replace("\r\n ", " ")
        .replace("\r\n\t", " ")
        .replace("\n ", " ")
        .replace("\n\t", " ");
    let value = unfolded.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("content-type")
            .then_some(value)
    })?;

    let mut params = value.split(';');
    let media_type = params.next()?.trim();
    if !media_type
        .get(..10)
        .is_some_and(|t| t.eq_ignore_ascii_case("multipart/"))
    {
        return None;
    }

    params.find_map(|param| {
        let (name, value) = param.split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("boundary")
            .then(|| value.trim().trim_matches('"').to_owned())
    })
}

/// The parts between `--boundary` lines, each without the line break that
/// belongs to the next delimiter.
fn parts<'a>(body: &'a [u8], boundary: &str) -> Vec<&'a [u8]> {
    let delimiter = format!("--{boundary}");
    let mut parts = Vec::new();
    let mut start = None;
    let mut at = 0;
    while at < body.len() {
        let end = body[at..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(body.len(), |n| at + n);
        let line = &body[at..end];
        // Only padding may follow a delimiter: a nested boundary can start
        // with this one.
        let rest = line
            .strip_prefix(delimiter.as_bytes())
            .filter(|rest| rest.starts_with(b"--") || rest.iter().all(u8::is_ascii_whitespace));
        if let Some(rest) = rest {
            if let Some(start) = start {
                let content = &body[start..at];
                let content = content.strip_suffix(b"\n").unwrap_or(content);
                parts.push(content.strip_suffix(b"\r").unwrap_or(content));
            }

            if rest.starts_with(b"--") {
                return parts;
            }

            start = Some((end + 1).min(body.len()));
        }

        at = end + 1;
    }

    // A Message cut short loses only its closing delimiter.
    if let Some(start) = start {
        parts.push(&body[start..]);
    }

    parts
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
