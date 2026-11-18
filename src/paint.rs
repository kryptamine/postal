use crate::rgb::Rgb;

/// The colours a run of rendered mail keeps from the mail, already moved
/// onto the terminal's background; `None` is the terminal's own colour.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Paint {
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
}
