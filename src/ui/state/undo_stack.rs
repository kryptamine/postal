use crate::ui::state::undo::Undo;

/// What an Undo may take back, the latest last.
#[derive(Default)]
pub struct UndoStack(Vec<Undo>);

impl UndoStack {
    pub fn push(&mut self, undo: Undo) {
        self.0.push(undo);
    }

    pub fn pop(&mut self) -> Option<Undo> {
        self.0.pop()
    }

    /// Drops the Undo of each Send that left the Outbox: `u` never unsends
    /// what SMTP has, and moves on to the entry before.
    pub fn forget_sent(&mut self) {
        self.0
            .retain(|undo| !matches!(undo, Undo::Send(path) if !path.exists()));
    }

    pub const fn count(&self) -> usize {
        self.0.len()
    }
}
