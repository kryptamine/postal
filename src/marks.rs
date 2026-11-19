/// What a run of Preview text is, never how it looks: the Theme gives each
/// mark its style when the run is drawn, so `NO_COLOR` reaches it too.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Marks(u8);

impl Marks {
    pub const STRONG: Self = Self(1);
    pub const EMPHASIS: Self = Self(1 << 1);
    pub const LINK: Self = Self(1 << 2);
    pub const CODE: Self = Self(1 << 3);
    /// The alt text of an image inside a link.
    pub const ALT: Self = Self(1 << 4);
    /// Underlined by the mail's own CSS, in the rendered View.
    pub const UNDERLINE: Self = Self(1 << 5);
    /// A grey border of the rendered View, drawn like Postal's own.
    pub const BORDER: Self = Self(1 << 6);
    /// The footer of a mail in the rendered View.
    pub const MUTED: Self = Self(1 << 7);

    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn has(self, mark: Self) -> bool {
        self.0 & mark.0 != 0
    }
}
