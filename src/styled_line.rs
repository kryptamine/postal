use crate::marks::Marks;

/// One line of a Preview body: its text, and the runs that cover it end to
/// end, each with its marks.
pub struct StyledLine {
    pub text: String,
    runs: Vec<Run>,
}

struct Run {
    /// In bytes of `text`.
    len: usize,
    marks: Marks,
}

impl StyledLine {
    pub const fn new() -> Self {
        Self {
            text: String::new(),
            runs: Vec::new(),
        }
    }

    pub fn plain(text: String) -> Self {
        let runs = vec![Run {
            len: text.len(),
            marks: Marks::default(),
        }];

        Self { text, runs }
    }

    pub fn push(&mut self, text: &str, marks: Marks) {
        if text.is_empty() {
            return;
        }

        self.text.push_str(text);
        match self.runs.last_mut() {
            Some(last) if last.marks == marks => last.len += text.len(),
            _ => self.runs.push(Run {
                len: text.len(),
                marks,
            }),
        }
    }

    pub fn trim_end(&mut self) {
        let mut cut = self.text.len() - self.text.trim_end().len();
        self.text.truncate(self.text.len() - cut);
        while cut > 0 {
            let Some(last) = self.runs.last_mut() else {
                break;
            };

            let taken = cut.min(last.len);
            last.len -= taken;
            cut -= taken;
            if last.len == 0 {
                self.runs.pop();
            }
        }
    }

    /// The text in pieces of one set of marks each, in order.
    pub fn segments(&self) -> impl Iterator<Item = (&str, Marks)> {
        let mut start = 0;
        self.runs.iter().map(move |run| {
            let piece = &self.text[start..start + run.len];
            start += run.len;
            (piece, run.marks)
        })
    }

    /// The heap bytes it holds, near enough for a byte-bounded cache.
    pub const fn bytes(&self) -> usize {
        size_of::<Self>() + self.text.len() + self.runs.len() * size_of::<Run>()
    }
}
