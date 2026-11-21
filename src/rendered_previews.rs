use std::collections::HashMap;

use crate::{body_key::BodyKey, preview::Preview, view::View};

/// The bytes of rendered Previews kept (ADR 0001); past it the least
/// recently used go first.
const BYTES: usize = 8 * 1024 * 1024;

/// A Preview is rendered for one width and View, with its Quote or
/// without: a resize, or a switch of either, renders it again.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreviewKey {
    /// The Account the body key is of: keys repeat across Accounts.
    pub account: usize,
    pub body: BodyKey,
    pub width: u16,
    pub view: View,
    /// The Quote is shown.
    pub quoted: bool,
}

/// Rendered Previews in a byte-bounded LRU. Looking one up does not
/// allocate, so a frame may do it.
#[derive(Default)]
pub struct RenderedPreviews {
    entries: HashMap<PreviewKey, Entry>,
    bytes: usize,
    clock: u64,
}

struct Entry {
    preview: Preview,
    used: u64,
}

impl RenderedPreviews {
    pub fn get(&self, key: PreviewKey) -> Option<&Preview> {
        self.entries.get(&key).map(|e| &e.preview)
    }

    /// Marks the Preview as just used; `false` when it is not here.
    pub fn touch(&mut self, key: PreviewKey) -> bool {
        self.clock += 1;
        self.entries
            .get_mut(&key)
            .map(|entry| entry.used = self.clock)
            .is_some()
    }

    pub fn insert(&mut self, key: PreviewKey, preview: Preview) {
        self.clock += 1;
        self.bytes += preview.bytes();
        let entry = Entry {
            preview,
            used: self.clock,
        };

        if let Some(old) = self.entries.insert(key, entry) {
            self.bytes -= old.preview.bytes();
        }

        while self.bytes > BYTES && self.entries.len() > 1 {
            let Some((&oldest, _)) = self.entries.iter().min_by_key(|(_, e)| e.used) else {
                break;
            };

            if let Some(gone) = self.entries.remove(&oldest) {
                self.bytes -= gone.preview.bytes();
            }
        }
    }
}
