use std::{panic, sync::Arc};

use crate::{render::mail_look::MailLook, render::rendered_view, render::styled_line::StyledLine};

/// An HTML document of Postal's own to lay out in the Rendered View on
/// the render thread, such as a Markdown Draft's.
pub struct HtmlJob {
    pub html: Arc<str>,
    pub width: u16,
    pub look: MailLook,
}

/// What a job laid out, named by its HTML and width; `rows` is `None`
/// where the engine failed or panicked on it.
pub struct LaidOut {
    pub html: Arc<str>,
    pub width: u16,
    pub rows: Option<Vec<StyledLine>>,
}

impl HtmlJob {
    /// Catches a panic, so it runs off the main thread only: the panic
    /// hook restores the terminal for a panic there, caught or not.
    pub fn run(self) -> LaidOut {
        let rows = panic::catch_unwind(|| rendered_view::render(&self.html, self.width, self.look));
        LaidOut {
            rows: rows.ok().flatten(),
            html: self.html,
            width: self.width,
        }
    }
}
