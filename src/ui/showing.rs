use crate::{ui::mode::Mode, ui::reading::Reading};

/// What the panes show under whatever is open over them.
#[derive(Default)]
pub enum Showing {
    /// The Message List, with the Preview beside or below it.
    #[default]
    List,
    /// The Message List, while the Query is typed in the hint line.
    Query,
    /// The selected Thread fills the screen as a stack of its Messages.
    Reading(Reading),
}

impl Showing {
    pub const fn mode(&self) -> Mode {
        match self {
            Self::List => Mode::List,
            Self::Query => Mode::Query,
            Self::Reading(_) => Mode::Reading,
        }
    }

    pub const fn reading(&self) -> Option<&Reading> {
        match self {
            Self::Reading(reading) => Some(reading),
            Self::List | Self::Query => None,
        }
    }

    pub const fn reading_mut(&mut self) -> Option<&mut Reading> {
        match self {
            Self::Reading(reading) => Some(reading),
            Self::List | Self::Query => None,
        }
    }
}
