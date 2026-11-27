/// Decodes RFC 2047 encoded words; undecodable bytes are kept lossily.
/// Folds are unfolded and any other control character becomes a space,
/// so a header always fits one row.
pub fn decode(raw: &[u8]) -> String {
    let text =
        rfc2047_decoder::decode(raw).unwrap_or_else(|_| String::from_utf8_lossy(raw).into_owned());
    text.replace("\r\n ", " ")
        .replace("\r\n\t", " ")
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// The value of the field `name` in the header block `header`, its case
/// ignored, as sent: folds kept.
pub fn field<'a>(header: &'a [u8], name: &str) -> Option<&'a [u8]> {
    let mut rest = header;
    while !rest.is_empty() {
        // A field ends at a line break not followed by a fold.
        let mut end = 0;
        let end = loop {
            match rest[end..].windows(2).position(|pair| pair == b"\r\n") {
                Some(at)
                    if rest
                        .get(end + at + 2)
                        .is_some_and(|b| matches!(b, b' ' | b'\t')) =>
                {
                    end += at + 2;
                }
                Some(at) => break end + at,
                None => break rest.len(),
            }
        };

        let (line, next) = (&rest[..end], rest.get(end + 2..).unwrap_or_default());
        if let Some(colon) = line.iter().position(|&b| b == b':')
            && line[..colon]
                .trim_ascii()
                .eq_ignore_ascii_case(name.as_bytes())
        {
            return Some(&line[colon + 1..]);
        }

        rest = next;
    }

    None
}
