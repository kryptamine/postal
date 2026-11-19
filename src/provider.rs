/// Where an Account's mail lives, and so how it signs in.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Google,
}

impl Provider {
    pub const ALL: [Self; 1] = [Self::Google];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Google => "Google (Gmail)",
        }
    }

    /// Whose sign-in page the browser shows.
    pub const fn service(self) -> &'static str {
        match self {
            Self::Google => "Google",
        }
    }
}
