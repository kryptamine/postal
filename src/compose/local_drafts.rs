use std::io;

use crate::{
    compose::local_draft::{self, LocalDraft},
    compose::outbox::Outbox,
};

/// Local Drafts as shown: every Draft and Outbox Message, and the row
/// selected.
pub struct LocalDrafts {
    rows: Vec<LocalDraft>,
    selected: usize,
}

impl LocalDrafts {
    /// Lists the Drafts, and the Messages of `outbox`, the newest selected.
    pub fn open(outbox: &Outbox) -> io::Result<Self> {
        Ok(Self {
            rows: local_draft::list(outbox)?,
            selected: 0,
        })
    }

    /// Lists them anew, the selected one kept while it is listed.
    pub fn reload(&mut self, outbox: &Outbox) -> io::Result<()> {
        let rows = local_draft::list(outbox)?;
        let chosen = self.chosen().map(|row| row.path.clone());
        self.selected = chosen
            .and_then(|path| rows.iter().position(|row| row.path == path))
            .unwrap_or_else(|| self.selected.min(rows.len().saturating_sub(1)));
        self.rows = rows;
        Ok(())
    }

    pub fn down(&mut self) {
        self.selected = (self.selected + 1).min(self.rows.len().saturating_sub(1));
    }

    pub const fn up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn chosen(&self) -> Option<&LocalDraft> {
        self.rows.get(self.selected)
    }

    pub const fn selected(&self) -> usize {
        self.selected
    }

    pub fn rows(&self) -> &[LocalDraft] {
        &self.rows
    }
}
