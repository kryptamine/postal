/// A Message rendered for the Preview at one width: the header values and
/// the body already wrapped, so drawing it only copies lines.
pub struct Preview {
    pub from: String,
    pub subject: String,
    pub date: String,
    pub body: Vec<String>,
}

impl Preview {
    /// The heap bytes it holds, near enough for a byte-bounded cache.
    pub fn bytes(&self) -> usize {
        let line = size_of::<String>();
        self.from.len()
            + self.subject.len()
            + self.date.len()
            + self.body.iter().map(|l| l.len() + line).sum::<usize>()
    }
}
