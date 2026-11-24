/// The `name:` a term of a Query starts with.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    From,
    To,
    Cc,
    Subject,
    Is,
    Has,
    In,
    Account,
    Before,
    After,
    Older,
    Newer,
}

impl Operator {
    pub const ALL: [Self; 12] = [
        Self::From,
        Self::To,
        Self::Cc,
        Self::Subject,
        Self::Is,
        Self::Has,
        Self::In,
        Self::Account,
        Self::Before,
        Self::After,
        Self::Older,
        Self::Newer,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::From => "from",
            Self::To => "to",
            Self::Cc => "cc",
            Self::Subject => "subject",
            Self::Is => "is",
            Self::Has => "has",
            Self::In => "in",
            Self::Account => "account",
            Self::Before => "before",
            Self::After => "after",
            Self::Older => "older",
            Self::Newer => "newer",
        }
    }

    /// In any case.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|operator| operator.name().eq_ignore_ascii_case(name))
    }
}
