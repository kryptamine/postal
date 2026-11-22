/// How Reading shows one line of its stack.
#[derive(Clone, Copy)]
pub enum StackLine {
    /// One line standing for this many folded Messages in a row.
    Run(usize),
    Folded,
    Unfolded,
}

impl StackLine {
    /// How many Messages of the stack it shows.
    pub const fn messages(self) -> usize {
        match self {
            Self::Run(count) => count,
            Self::Folded | Self::Unfolded => 1,
        }
    }
}
