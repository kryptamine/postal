/// The key a Message's body is cached under: Gmail's `X-GM-MSGID`, which
/// survives a UIDVALIDITY change, else the UID under its UIDVALIDITY.
/// See docs/adr/0005-body-cache-key.md.
pub fn body_key(gmail_id: Option<u64>, uid_validity: u32, uid: u32) -> i64 {
    gmail_id
        .unwrap_or_else(|| (u64::from(uid_validity) << 32) | u64::from(uid))
        .cast_signed()
}
