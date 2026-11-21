use crate::render::styled_line::StyledLine;

/// A Message's body rendered for the Preview at one width, already
/// wrapped, so drawing it only copies lines.
pub struct Preview {
    pub body: Vec<StyledLine>,
    /// The body keeps the mail's layout, in the rendered View.
    pub laid_out: bool,
}

impl Preview {
    /// The heap bytes it holds, near enough for a byte-bounded cache.
    pub fn bytes(&self) -> usize {
        self.body.iter().map(StyledLine::bytes).sum::<usize>()
    }
}
