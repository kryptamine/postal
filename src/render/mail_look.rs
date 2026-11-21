use crate::{render::mail_colours::MailColours, ui::icons::Icons};

/// How the rendered View draws mail on this terminal: its colours moved
/// onto the terminal's background, and the glyphs the font has.
#[derive(Clone, Copy)]
pub struct MailLook {
    pub colours: MailColours,
    pub icons: Icons,
}
