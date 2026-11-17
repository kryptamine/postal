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
