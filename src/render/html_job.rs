use std::sync::Arc;

use crate::{render::mail_look::MailLook, render::rendered_view, render::styled_line::StyledLine};

/// An HTML document of Postal's own to lay out in the Rendered View on
/// the render thread, such as a Markdown Draft's.
pub struct HtmlJob {
    pub html: Arc<str>,
    pub width: u16,
    pub look: MailLook,
}

/// What a job laid out, named by its HTML and width; `rows` is `None`
/// where the engine failed on it.
pub struct LaidOut {
    pub html: Arc<str>,
    pub width: u16,
    pub rows: Option<Vec<StyledLine>>,
}

impl HtmlJob {
    pub fn run(self) -> LaidOut {
        LaidOut {
            rows: rendered_view::render(&self.html, self.width, self.look),
            html: self.html,
            width: self.width,
        }
    }
}
