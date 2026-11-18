/// What the main screen shows, and so what the keys do there.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    /// The Message List, with the Preview beside or below it.
    #[default]
    List,
    /// The selected Message's Preview fills the screen.
    Reading,
    /// The Message List, while the Query is typed in the hint line.
    Query,
}
