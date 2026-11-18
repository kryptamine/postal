use std::sync::Arc;

use crate::marks::Marks;

/// One line of a Preview body: its text, and the runs that cover it end to
/// end, each with its marks and the URL of the link it is part of.
pub struct StyledLine {
    pub text: String,
    runs: Vec<Run>,
}

struct Run {
    /// In bytes of `text`.
    len: usize,
    marks: Marks,
    link: Option<Arc<str>>,
}

/// A run of text with one set of marks, inside at most one link.
pub struct Segment<'a> {
    pub text: &'a str,
    pub marks: Marks,
    pub link: Option<&'a Arc<str>>,
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
            link: None,
        }];

        Self { text, runs }
    }

    pub fn push(&mut self, text: &str, marks: Marks, link: Option<&Arc<str>>) {
        if text.is_empty() {
            return;
        }

        self.text.push_str(text);
        match self.runs.last_mut() {
            Some(last) if last.marks == marks && last.link.as_ref() == link => {
                last.len += text.len();
            }
            _ => self.runs.push(Run {
                len: text.len(),
                marks,
                link: link.cloned(),
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

    /// The text in runs, in order.
    pub fn segments(&self) -> impl Iterator<Item = Segment<'_>> {
        let mut start = 0;
        self.runs.iter().map(move |run| {
            let text = &self.text[start..start + run.len];
            start += run.len;
            Segment {
                text,
                marks: run.marks,
                link: run.link.as_ref(),
            }
        })
    }

    /// The heap bytes it holds, near enough for a byte-bounded cache. A URL
    /// is shared by its runs, and counted once per run.
    pub fn bytes(&self) -> usize {
        let links: usize = self
            .runs
            .iter()
            .filter_map(|run| run.link.as_ref())
            .map(|url| url.len())
            .sum();
        size_of::<Self>() + self.text.len() + self.runs.len() * size_of::<Run>() + links
    }
}
