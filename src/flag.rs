/// A flag Postal writes on the server.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    Seen,
    /// Set once a Reply to the Message is sent.
    Answered,
}

impl Flag {
    /// As IMAP names it, and as the Cache's queue stores it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Seen => "\\Seen",
            Self::Answered => "\\Answered",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Seen, Self::Answered]
            .into_iter()
            .find(|flag| flag.name() == name)
    }
}
