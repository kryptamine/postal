use std::fmt;

/// The key a Message's body is cached under: Gmail's `X-GM-MSGID`, which
/// survives a UIDVALIDITY change, else the UID under its UIDVALIDITY.
/// See docs/adr/0005-body-cache-key.md.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyKey(i64);

impl BodyKey {
    pub fn new(gmail_id: Option<u64>, uid_validity: u32, uid: u32) -> Self {
        Self(
            gmail_id
                .unwrap_or_else(|| (u64::from(uid_validity) << 32) | u64::from(uid))
                .cast_signed(),
        )
    }

    /// The key as the Cache stored it.
    pub const fn from_stored(key: i64) -> Self {
        Self(key)
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for BodyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
