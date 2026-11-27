use std::fmt;

/// Why a part asked for will not come.
pub enum Unfetched {
    /// The connection it was sent to dropped first.
    Offline,
    /// The server refused it, or no longer has it.
    Failed(String),
}

impl fmt::Display for Unfetched {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Offline => f.write_str("offline"),
            Self::Failed(reason) => f.write_str(reason),
        }
    }
}
