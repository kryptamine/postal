use crate::{ui::layer::Layer, ui::step::Step, ui::toast::Toast};

/// What a key in the top layer leaves to the screen.
pub enum Act {
    Stay,
    Help,
    /// The layer closes, then the event loop takes the step.
    Close(Step),
    Step(Step),
    /// Another layer opens over this one.
    Open(Box<Layer>),
    Toast(Toast),
}
