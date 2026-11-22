use std::{ops::Range, sync::Arc};

use crossterm::event::KeyEvent;

use crate::{
    account::browser,
    render::stack_str::StackStr,
    render::styled_line::StyledLine,
    ui::preview::hint_keys::{self, HintKeys},
};

/// Hint mode: each distinct URL in the bodies on screen, in reading order,
/// labelled before the first run of it shown.
pub struct LinkHints {
    targets: Vec<Target>,
    keys: HintKeys,
}

struct Target {
    /// The body, the line in it and the run in that the label goes before.
    part: usize,
    line: usize,
    run: usize,
    url: Arc<str>,
}

/// What one key does in hint mode; every key ends it but a label's first
/// letter.
pub enum Typed {
    Partial,
    Open(Arc<str>),
    Cancel,
}

impl LinkHints {
    /// The links on each `(part, body, lines)` shown, in that order; `None`
    /// when there is none.
    pub fn find<'a>(
        bodies: impl IntoIterator<Item = (usize, &'a [StyledLine], Range<usize>)>,
    ) -> Option<Self> {
        let mut targets: Vec<Target> = Vec::new();
        let shown = bodies.into_iter().flat_map(|(part, body, lines)| {
            let lines = body.iter().enumerate().take(lines.end).skip(lines.start);
            lines.map(move |(line, styled)| (part, line, styled))
        });
        for (part, line, styled) in shown {
            for (run, segment) in styled.segments().enumerate() {
                let Some(url) = segment.link.filter(|url| browser::safe(url)) else {
                    continue;
                };

                if targets.len() < hint_keys::MOST && targets.iter().all(|t| t.url != *url) {
                    targets.push(Target {
                        part,
                        line,
                        run,
                        url: Arc::clone(url),
                    });
                }
            }
        }

        (!targets.is_empty()).then(|| Self {
            keys: HintKeys::new(targets.len()),
            targets,
        })
    }

    /// The label drawn before run `run` of line `line` of body `part`, if any.
    pub fn label(&self, part: usize, line: usize, run: usize) -> Option<StackStr<2>> {
        let index = self
            .targets
            .iter()
            .position(|t| t.part == part && t.line == line && t.run == run)?;
        self.keys.label(index)
    }

    pub fn key(&mut self, key: KeyEvent) -> Typed {
        match self.keys.key(key) {
            hint_keys::Typed::Partial => Typed::Partial,
            hint_keys::Typed::Picked(index) => Typed::Open(Arc::clone(&self.targets[index].url)),
            hint_keys::Typed::Cancel => Typed::Cancel,
        }
    }
}
