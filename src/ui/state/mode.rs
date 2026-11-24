/// What the keys do: the panes' `Showing`, or the layer open over them.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    /// The Message List, with the Preview beside or below it.
    #[default]
    List,
    /// The selected Thread fills the screen as a stack of its Messages.
    Reading,
    /// The Message List, while the Query is typed in the hint line.
    Query,
    /// As `Query`, with `Tab` gone into the menu over the Query line.
    Menu,
    /// A Draft's preview over the screen. It names the keys only: the
    /// screen under it keeps its own mode.
    Draft,
    /// Local Drafts over the screen.
    LocalDrafts,
    /// An Outbox Message over the screen, read-only.
    Queued,
    /// Settings over the screen.
    Settings,
}

impl Mode {
    /// The Query is being typed, in the menu or not.
    pub const fn typing(self) -> bool {
        matches!(self, Self::Query | Self::Menu)
    }
}
