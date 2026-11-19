use std::sync::Arc;

use crate::{marks::Marks, paint::Paint};

/// One line of a Preview body: its text, and the runs that cover it end to
/// end, each with its marks, its colours and the URL of the link it is in.
#[derive(PartialEq, Eq)]
pub struct StyledLine {
    pub text: String,
    runs: Vec<Run>,
}

#[derive(PartialEq, Eq)]
struct Run {
    /// In bytes of `text`.
    len: usize,
    marks: Marks,
    paint: Paint,
    link: Option<Arc<str>>,
}

/// A run of text with one set of marks and colours, inside at most one link.
pub struct Segment<'a> {
    pub text: &'a str,
    pub marks: Marks,
    pub paint: Paint,
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
            paint: Paint::default(),
            link: None,
        }];

        Self { text, runs }
    }

    pub fn push(&mut self, text: &str, marks: Marks, paint: Paint, link: Option<&Arc<str>>) {
        if text.is_empty() {
            return;
        }

        self.text.push_str(text);
        match self.runs.last_mut() {
            Some(last)
                if last.marks == marks && last.paint == paint && last.link.as_ref() == link =>
            {
                last.len += text.len();
            }
            _ => self.runs.push(Run {
                len: text.len(),
                marks,
                paint,
                link: link.cloned(),
            }),
        }
    }

    /// Drops trailing whitespace, but not where a background colour shows it.
    pub fn trim_end(&mut self) {
        let painted = self
            .runs
            .iter()
            .rposition(|run| run.paint.bg.is_some())
            .map_or(0, |last| self.runs[..=last].iter().map(|run| run.len).sum());
        let kept = self.text.trim_end().len().max(painted);
        let mut cut = self.text.len() - kept;
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

    /// Every run as `marks` alone, in the terminal's own colours, still in
    /// its link. A fill gone leaves nothing to show the blanks at the end.
    pub fn restyle(&mut self, marks: Marks) {
        for run in &mut self.runs {
            run.marks = marks;
            run.paint = Paint::default();
        }

        self.trim_end();
    }

    /// Shows nothing but the sides of the frames it crosses: no word, and
    /// no fill, which would make the row part of a drawing.
    pub fn spacer(&self) -> bool {
        let blank = |c| c == ' ' || c == '│';
        self.text.chars().all(blank) && self.runs.iter().all(|run| run.paint.bg.is_none())
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
                paint: run.paint,
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
