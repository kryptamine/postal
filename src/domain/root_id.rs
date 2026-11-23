/// A Message's root, hashed: the first `Message-ID` of `References`, else
/// of `In-Reply-To`, else its own. Names a Thread without `X-GM-THRID` (ADR 0009).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RootId(u64);

impl RootId {
    /// Of the `Message-ID` `id`, without its angle brackets. FNV-1a, as the
    /// Cache keeps it: the hash must not change between builds.
    pub fn of(id: &[u8]) -> Self {
        let hash = id.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, &byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
        Self(hash)
    }

    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}
