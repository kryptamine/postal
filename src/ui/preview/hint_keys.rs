use std::fmt::Write;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::render::stack_str::StackStr;

/// The home row, so typing a label needs no reach.
const ALPHABET: [char; 9] = ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'];
/// Every two-letter label; targets past this many get none.
pub const MOST: usize = ALPHABET.len() * ALPHABET.len();

/// The labels of hint mode over `count` targets, all one length, and the
/// letter typed so far of a two-letter one.
pub struct HintKeys {
    count: usize,
    first: Option<usize>,
}

/// What one key does in hint mode; every key ends it but a label's first
/// letter.
pub enum Typed {
    Partial,
    Picked(usize),
    Cancel,
}

impl HintKeys {
    pub fn new(count: usize) -> Self {
        Self {
            count: count.min(MOST),
            first: None,
        }
    }

    /// The label of target `index`; `None` past the last.
    pub fn label(&self, index: usize) -> Option<StackStr<2>> {
        if index >= self.count {
            return None;
        }

        let mut label = StackStr::new();
        if self.two_letters() {
            label.write_char(ALPHABET[index / ALPHABET.len()]).ok();
        }

        label.write_char(ALPHABET[index % ALPHABET.len()]).ok();
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
                if letter * ALPHABET.len() >= self.count {
                    return Typed::Cancel;
                }

                self.first = Some(letter);
                return Typed::Partial;
            }
            None => letter,
            Some(first) => first * ALPHABET.len() + letter,
        };

        if index < self.count {
            Typed::Picked(index)
        } else {
            Typed::Cancel
        }
    }

    const fn two_letters(&self) -> bool {
        self.count > ALPHABET.len()
    }
}
