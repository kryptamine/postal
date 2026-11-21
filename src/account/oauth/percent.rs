use std::fmt::Write;

/// `text` percent-encoded for a URL query, every byte but the unreserved ones.
pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            // Writing to a String cannot fail.
            write!(out, "%{byte:02X}").ok();
        }
    }

    out
}

/// A query value decoded, `+` as a space; `None` for a broken escape or
/// bytes that are not UTF-8.
pub fn decode(text: &str) -> Option<String> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.bytes();
    while let Some(byte) = bytes.next() {
        match byte {
            b'%' => {
                let high = char::from(bytes.next()?).to_digit(16)?;
                let low = char::from(bytes.next()?).to_digit(16)?;
                out.push(u8::try_from(high * 16 + low).ok()?);
            }
            b'+' => out.push(b' '),
            byte => out.push(byte),
        }
    }

    String::from_utf8(out).ok()
}
