use crossterm::event::KeyEvent;

use crate::{
    ui::input::step::Step,
    ui::popup::layer_act::Act,
    ui::preview::attachment_hints::{AttachmentHints, Picked},
    ui::preview::link_hints::{LinkHints, Typed},
};

/// Every key ends hint mode but a label's first letter.
pub fn key(hints: &mut LinkHints, key: KeyEvent) -> Act {
    match hints.key(key) {
        Typed::Partial => Act::Stay,
        Typed::Open(url) => Act::Close(Step::Browse(url)),
        Typed::Cancel => Act::Close(Step::Stay),
    }
}

/// As `key`, on the attachment line.
pub fn attachment_key(hints: &mut AttachmentHints, key: KeyEvent) -> Act {
    match hints.key(key) {
        Picked::Partial => Act::Stay,
        Picked::Wanted(wanted) => Act::Close(Step::Attachment(wanted)),
        Picked::Cancel => Act::Close(Step::Stay),
    }
}
