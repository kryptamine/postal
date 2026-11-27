use crate::account::written::written;

written! {
    /// Where an Account's mail lives, and so how it signs in.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub enum Provider {
        #[default]
        Google = "google",
        ICloud = "icloud",
        /// Any IMAP/SMTP server, its hosts written in the Account's tables.
        Other = "other",
    }
}

impl Provider {
    pub const ALL: [Self; 3] = [Self::Google, Self::ICloud, Self::Other];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Google => "Google (Gmail)",
            Self::ICloud => "iCloud",
            Self::Other => "Other (IMAP)",
        }
    }

    /// The service the user signs in to, as the sign-in steps name it.
    pub const fn service(self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::ICloud => "iCloud",
            Self::Other => "IMAP",
        }
    }
}
