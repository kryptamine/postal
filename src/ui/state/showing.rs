use crate::{domain::body_key::BodyKey, ui::preview::reading::Reading, ui::state::mode::Mode};

/// What the panes show under whatever is open over them.
#[derive(Default)]
pub enum Showing {
    /// The Message List, with the Preview beside or below it.
    #[default]
    List,
    /// The Message List, while the Query is typed in the hint line.
    Query,
    /// As `Query`, a suggestion of the menu written in.
    Menu,
    /// The selected Thread fills the screen as a stack of its Messages.
    Reading(Reading),
}

impl Showing {
    pub const fn mode(&self) -> Mode {
        match self {
            Self::List => Mode::List,
            Self::Query => Mode::Query,
            Self::Menu => Mode::Menu,
            Self::Reading(_) => Mode::Reading,
        }
    }

    pub const fn reading(&self) -> Option<&Reading> {
        match self {
            Self::Reading(reading) => Some(reading),
            Self::List | Self::Query | Self::Menu => None,
        }
    }

    /// The Message Reading points at; `None` outside Reading.
    pub fn cursor(&self) -> Option<BodyKey> {
        self.reading().map(|reading| reading.cursor)
    }

    pub const fn reading_mut(&mut self) -> Option<&mut Reading> {
        match self {
            Self::Reading(reading) => Some(reading),
            Self::List | Self::Query | Self::Menu => None,
        }
    }
}
