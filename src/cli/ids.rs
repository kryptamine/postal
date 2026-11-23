//! The IDs the CLI and the MCP server hand out for a Message and a Thread:
//! derived from the Cache's keys, never stored (ADR 0030).

use crate::{
    domain::body_key::BodyKey, domain::root_id::RootId, domain::thread::Key,
    domain::thread_id::ThreadId,
};

/// Crockford's base32, lowercased: no `i`, `l`, `o` or `u` to misread.
const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// Characters of an Account's tag: 20 bits, so a few Accounts never share one.
const TAG: usize = 4;

/// Characters of a 64-bit key.
const KEY: usize = 13;

pub fn message(account: &str, key: BodyKey) -> String {
    let mut id = String::from("m");
    push(&mut id, tag(account).into(), TAG);
    push(&mut id, key.get().cast_unsigned(), KEY);
    id
}

pub fn thread(key: &Key<String>) -> String {
    let (kind, bits) = match *key {
        Key::Gmail(_, id) => ('g', id.get()),
        Key::Root(_, root) => ('r', root.get()),
        Key::Lone(_, body) => ('l', body.get().cast_unsigned()),
    };

    let mut id = String::from("t");
    id.push(kind);
    push(&mut id, tag(key.account()).into(), TAG);
    push(&mut id, bits, KEY);
    id
}

/// The Account of `accounts` and the key a Message ID names.
pub fn parse_message<'a>(id: &str, accounts: &'a [String]) -> Result<(&'a str, BodyKey), String> {
    let rest = id
        .strip_prefix('m')
        .ok_or_else(|| format!("{id} is no Message ID"))?;
    let (account, bits) = parts(id, rest, accounts)?;
    Ok((account, BodyKey::from_stored(bits.cast_signed())))
}

/// The Thread a Thread ID names, in one of `accounts`.
pub fn parse_thread(id: &str, accounts: &[String]) -> Result<Key<String>, String> {
    let wrong = || format!("{id} is no Thread ID");
    let rest = id.strip_prefix('t').ok_or_else(wrong)?;
    let mut chars = rest.chars();
    let kind = chars.next().ok_or_else(wrong)?;
    let (account, bits) = parts(id, chars.as_str(), accounts)?;
    let account = account.to_owned();
    match kind {
        'g' => Ok(Key::Gmail(account, ThreadId::new(bits))),
        'r' => Ok(Key::Root(account, RootId::new(bits))),
        'l' => Ok(Key::Lone(account, BodyKey::from_stored(bits.cast_signed()))),
        _ => Err(wrong()),
    }
}

/// The tag and the key after an ID's kind, the tag as the Account it names.
fn parts<'a>(id: &str, rest: &str, accounts: &'a [String]) -> Result<(&'a str, u64), String> {
    let wrong = || format!("{id} is no Postal ID");
    if rest.len() != TAG + KEY {
        return Err(wrong());
    }

    let (tagged, key) = rest.split_at(TAG);
    let tagged = decode(tagged).ok_or_else(wrong)?;
    let bits = decode(key).ok_or_else(wrong)?;
    let mut named = accounts
        .iter()
        .filter(|account| u64::from(tag(account)) == tagged);
    let account = named
        .next()
        .ok_or_else(|| format!("{id} names no Account set up"))?;
    if named.next().is_some() {
        return Err(format!("{id} names more than one Account"));
    }

    Ok((account, bits))
}

/// 20 bits of the lowercased address, hashed as a `RootId` is.
fn tag(account: &str) -> u32 {
    let hash = RootId::of(account.to_lowercase().as_bytes()).get();
    // Keeps the low 20 bits, which fit in a `u32`.
    u32::try_from(hash & 0xf_ffff).unwrap_or_default()
}

/// The low `width * 5` bits of `bits`, most significant first.
fn push(into: &mut String, bits: u64, width: usize) {
    for at in (0..width).rev() {
        let digit = (bits >> (at * 5)) & 31;
        into.push(char::from(
            ALPHABET[usize::try_from(digit).unwrap_or_default()],
        ));
    }
}

/// `None` past 64 bits, so no two IDs name one key.
fn decode(text: &str) -> Option<u64> {
    text.bytes().try_fold(0_u64, |bits, byte| {
        let digit = ALPHABET.iter().position(|&c| c == byte)?;
        if bits >> 59 != 0 {
            return None;
        }

        Some((bits << 5) | u64::try_from(digit).ok()?)
    })
}
