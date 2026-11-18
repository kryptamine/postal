use std::borrow::Cow;

use base64::{
    Engine,
    alphabet::Alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};

// Base64 with `,` for `/` and no padding (RFC 3501 §5.1.3).
const BASE64: GeneralPurpose = GeneralPurpose::new(
    &match Alphabet::new("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+,") {
        Ok(alphabet) => alphabet,
        Err(_) => panic!("invalid alphabet"),
    },
    GeneralPurposeConfig::new()
        .with_encode_padding(false)
        .with_decode_padding_mode(DecodePaddingMode::RequireNone),
);

/// A mailbox name as people read it; a name that is not valid modified
/// UTF-7 comes back unchanged.
pub fn decode(name: &str) -> Cow<'_, str> {
    if !name.contains('&') {
        return Cow::Borrowed(name);
    }

    try_decode(name).map_or(Cow::Borrowed(name), Cow::Owned)
}

fn try_decode(name: &str) -> Option<String> {
    let mut decoded = String::with_capacity(name.len());
    let mut rest = name;
    while let Some(at) = rest.find('&') {
        decoded.push_str(&rest[..at]);
        let shifted = &rest[at + 1..];
        let end = shifted.find('-')?;
        let run = &shifted[..end];
        if run.is_empty() {
            decoded.push('&');
        } else {
            let bytes = BASE64.decode(run).ok()?;
            if bytes.len() % 2 != 0 {
                return None;
            }

            let units = bytes
                .chunks_exact(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]]));
            for c in char::decode_utf16(units) {
                decoded.push(c.ok()?);
            }
        }

        rest = &shifted[end + 1..];
    }

    decoded.push_str(rest);
    Some(decoded)
}
