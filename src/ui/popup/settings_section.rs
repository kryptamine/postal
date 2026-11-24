/// One tab of Settings.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Accounts,
    Icons,
    Notify,
    Name,
    Secrets,
}

impl Section {
    /// In the order the tabs show.
    pub const ALL: [Self; 5] = [
        Self::Accounts,
        Self::Icons,
        Self::Notify,
        Self::Name,
        Self::Secrets,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Accounts => "Accounts",
            Self::Icons => "Icons",
            Self::Notify => "Notify",
            Self::Name => "Name",
            Self::Secrets => "Secrets",
        }
    }

    /// The tab `step` places from this one, round the ends.
    pub fn moved(self, step: isize) -> Self {
        let at = Self::ALL.iter().position(|&s| s == self).unwrap_or(0);
        let count = Self::ALL.len().cast_signed();
        let to = (at.cast_signed() + step).rem_euclid(count);
        Self::ALL[to.cast_unsigned()]
    }
}
