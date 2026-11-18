use crate::{
    backfill::Backfill, body_fetch::Fetched, preview::Preview, rendered_previews::PreviewKey,
    status::Status,
};

/// Work done away from the event loop that changes what the screen shows.
pub enum Update {
    /// The sync wrote rows or Snippets to the Cache.
    Inbox,
    Bodies(Fetched),
    /// The connection for bodies is up, or has gone down.
    BodyLink(bool),
    Rendered(PreviewKey, Preview),
    Status(Status),
    Backfill(Backfill),
}
