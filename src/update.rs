use crate::{preview::Preview, rendered_previews::PreviewKey, status::Status};

/// Work done away from the event loop that changes what the screen shows.
pub enum Update {
    /// The sync wrote rows or Snippets to the Cache.
    Inbox,
    /// These UIDs' bodies are now in the Cache.
    Bodies(Vec<u32>),
    Rendered(PreviewKey, Preview),
    Status(Status),
}
