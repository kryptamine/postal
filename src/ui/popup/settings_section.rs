/// One tab of Settings.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Accounts,
    Icons,
    Notify,
    Name,
    Mailto,
    Secrets,
}

impl Section {
    /// In the order the tabs show.
    pub const ALL: [Self; 6] = [
        Self::Accounts,
        Self::Icons,
        Self::Notify,
        Self::Name,
        Self::Mailto,
        Self::Secrets,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Accounts => "Accounts",
            Self::Icons => "Icons",
            Self::Notify => "Notify",
            Self::Name => "Name",
            Self::Secrets => "Secrets",
            Self::Mailto => "Mailto",
        }
    }

    /// What the tab sets, under its heading.
    pub const fn about(self) -> &'static str {
        match self {
            Self::Accounts => "the Accounts Postal signs in to, each with its Signature",
            Self::Icons => "Nerd Font glyphs in the lists, or symbols any font draws",
            Self::Notify => "when new mail in an Inbox raises a desktop notification",
            Self::Name => "your name in the From line of the Messages you send",
            Self::Secrets => "where the passwords and tokens of the Accounts are kept",
            Self::Mailto => "whether a mailto: link in another app opens in Postal",
        }
    }

    /// The tab after this one, or before it, round the ends.
    pub fn next(self, forward: bool) -> Self {
        let count = Self::ALL.len();
        let at = Self::ALL.iter().position(|&s| s == self).unwrap_or(0);
        let step = if forward { 1 } else { count - 1 };
        Self::ALL[(at + step) % count]
    }
}
