use std::fmt;

/// A secret that `Debug` never prints, so no log line can carry it.
pub struct Credential(String);

impl Credential {
    pub const fn new(secret: String) -> Self {
        Self(secret)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}
