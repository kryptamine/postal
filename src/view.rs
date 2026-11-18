/// How the Preview shows an HTML part; `v` switches between the two.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub enum View {
    /// The mail's own layout on the cell grid: columns, colours, borders.
    #[default]
    Rendered,
    /// Text in reading order, wrapped to the pane.
    Text,
}

impl View {
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Rendered => Self::Text,
            Self::Text => Self::Rendered,
        }
    }
}
