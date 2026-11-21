use crossterm::event::KeyEvent;

use crate::{
    ui::layer_act::Act,
    ui::link_hints::{LinkHints, Typed},
    ui::step::Step,
};

/// Every key ends hint mode but a label's first letter.
pub fn key(hints: &mut LinkHints, key: KeyEvent) -> Act {
    match hints.key(key) {
        Typed::Partial => Act::Stay,
        Typed::Open(url) => Act::Close(Step::Browse(url)),
        Typed::Cancel => Act::Close(Step::Stay),
    }
}
