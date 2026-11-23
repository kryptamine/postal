use std::{fmt, str::FromStr};

/// A release's `major.minor.patch`, ordered as releases follow each other.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    pub fn running() -> Result<Self, String> {
        env!("CARGO_PKG_VERSION").parse()
    }
}

impl FromStr for Version {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let mut parts = text.split('.').map(str::parse);
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(Ok(major)), Some(Ok(minor)), Some(Ok(patch)), None) => Ok(Self {
                major,
                minor,
                patch,
            }),
            _ => Err(format!("{text} is no release version")),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}
