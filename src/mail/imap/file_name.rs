use std::borrow::Cow;

use charset::Charset;

use crate::mail::imap::header;

type Params<'a> = [(Cow<'a, str>, Cow<'a, str>)];

/// The parameter `key` of `params`: RFC 2231's `key*` or its continuations
/// `key*0`, `key*1*`… first, else `key` with its RFC 2047 words decoded.
pub fn find(params: &Params<'_>, key: &str) -> Option<String> {
    extended(params, key)
        .or_else(|| {
            params
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(key))
                .map(|(_, value)| header::decode(value.as_bytes()))
        })
        .filter(|name| !name.trim().is_empty())
}

/// The RFC 2231 value of `key`, its segments joined in order.
fn extended(params: &Params<'_>, key: &str) -> Option<String> {
    let whole = params.iter().any(|(name, _)| {
        name.strip_suffix('*')
            .is_some_and(|start| start.eq_ignore_ascii_case(key))
    });
    let mut segments: Vec<(u32, bool, &str)> = params
        .iter()
        .filter_map(|(name, value)| {
            let rest = name
                .get(..key.len())
                .filter(|start| start.eq_ignore_ascii_case(key))
                .and_then(|_| name.get(key.len()..))?
                .strip_prefix('*')?;
            // `key*` is one encoded segment, and stands alone beside continuations.
            if rest.is_empty() {
                return Some((0, true, &**value));
            } else if whole {
                return None;
            }

            let (number, encoded) = rest
                .strip_suffix('*')
                .map_or((rest, false), |number| (number, true));
            Some((number.parse().ok()?, encoded, &**value))
        })
        .collect();
    segments.sort_unstable_by_key(|&(number, ..)| number);
    let &(_, first_encoded, _) = segments.first()?;

    let mut charset = "utf-8";
    let mut bytes = Vec::new();
    for (index, (_, encoded, value)) in segments.into_iter().enumerate() {
        let mut value = value;
        // Only the first encoded segment names `charset'language'`.
        if index == 0 && first_encoded {
            let mut parts = value.splitn(3, '\'');
            if let (Some(named), Some(_), Some(rest)) = (parts.next(), parts.next(), parts.next()) {
                charset = named;
                value = rest;
            }
        }

        if encoded {
            percent_decode(value, &mut bytes);
        } else {
            bytes.extend_from_slice(value.as_bytes());
        }
    }

    Some(Charset::for_label(charset.as_bytes()).map_or_else(
        || String::from_utf8_lossy(&bytes).into_owned(),
        |charset| charset.decode_without_bom_handling(&bytes).0.into_owned(),
    ))
}

/// A broken escape is kept as sent.
fn percent_decode(value: &str, into: &mut Vec<u8>) {
    let bytes = value.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let escaped = bytes
            .get(at + 1..at + 3)
            .filter(|hex| bytes[at] == b'%' && hex.iter().all(u8::is_ascii_hexdigit))
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        if let Some(byte) = escaped {
            into.push(byte);
            at += 3;
        } else {
            into.push(bytes[at]);
            at += 1;
        }
    }
}
