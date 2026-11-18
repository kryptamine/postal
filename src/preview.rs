use crate::styled_line::StyledLine;

/// A Message rendered for the Preview at one width: the header values and
/// the body already wrapped, so drawing it only copies lines.
pub struct Preview {
    pub from: String,
    pub subject: String,
    pub date: String,
    pub body: Vec<StyledLine>,
    /// The body keeps the mail's layout, in the rendered View.
    pub laid_out: bool,
}

impl Preview {
    /// The heap bytes it holds, near enough for a byte-bounded cache.
    pub fn bytes(&self) -> usize {
        self.from.len()
            + self.subject.len()
            + self.date.len()
            + self.body.iter().map(StyledLine::bytes).sum::<usize>()
    }
}
