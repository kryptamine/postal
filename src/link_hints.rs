use std::{fmt::Write, ops::Range, sync::Arc};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{browser, stack_str::StackStr, styled_line::StyledLine};

/// The home row, so typing a label needs no reach.
const ALPHABET: [char; 9] = ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'];
/// Every two-letter label; links past this many get none.
const MOST: usize = ALPHABET.len() * ALPHABET.len();

/// Hint mode: each distinct URL in the bodies on screen, in reading order,
/// labelled before the first run of it shown. Labels are all one length.
pub struct LinkHints {
    targets: Vec<Target>,
    /// The letter typed so far of a two-letter label.
    first: Option<usize>,
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

                if targets.len() < MOST && targets.iter().all(|t| t.url != *url) {
                    targets.push(Target {
                        part,
                        line,
                        run,
                        url: Arc::clone(url),
                    });
                }
            }
        }

        (!targets.is_empty()).then_some(Self {
            targets,
            first: None,
        })
    }

    /// The label drawn before run `run` of line `line` of body `part`, if any.
    pub fn label(&self, part: usize, line: usize, run: usize) -> Option<StackStr<2>> {
        let index = self
            .targets
            .iter()
            .position(|t| t.part == part && t.line == line && t.run == run)?;
        let mut label = StackStr::new();
        if self.two_letters() {
            let _ = label.write_char(ALPHABET[index / ALPHABET.len()]);
        }

        let _ = label.write_char(ALPHABET[index % ALPHABET.len()]);
        Some(label)
    }

    pub fn key(&mut self, key: KeyEvent) -> Typed {
        let letter = match key.code {
            KeyCode::Char(c) if (key.modifiers - KeyModifiers::SHIFT).is_empty() => {
                ALPHABET.iter().position(|&a| a == c)
            }
            _ => None,
        };

        let Some(letter) = letter else {
            return Typed::Cancel;
        };

        let index = match self.first {
            None if self.two_letters() => {
                if letter * ALPHABET.len() >= self.targets.len() {
                    return Typed::Cancel;
                }

                self.first = Some(letter);
                return Typed::Partial;
            }
            None => letter,
            Some(first) => first * ALPHABET.len() + letter,
        };

        self.targets
            .get(index)
            .map_or(Typed::Cancel, |t| Typed::Open(Arc::clone(&t.url)))
    }

    const fn two_letters(&self) -> bool {
        self.targets.len() > ALPHABET.len()
    }
}
