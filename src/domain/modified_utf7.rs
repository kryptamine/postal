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
    while let Some((before, shifted)) = rest.split_once('&') {
        decoded.push_str(before);
        let (run, after) = shifted.split_once('-')?;
        if run.is_empty() {
            decoded.push('&');
        } else {
            let bytes = BASE64.decode(run).ok()?;
            if bytes.len() % 2 != 0 {
                return None;
            }

            let units = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_be_bytes(*pair));
            for c in char::decode_utf16(units) {
                decoded.push(c.ok()?);
            }
        }

        rest = after;
    }

    decoded.push_str(rest);
    Some(decoded)
}
