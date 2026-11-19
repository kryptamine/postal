use std::sync::Arc;

use crate::{
    backfill::Backfill, body_fetch::Fetched, preview::Preview, rendered_previews::PreviewKey,
    status::Status,
};

/// Work done away from the event loop that changes what the screen shows.
pub enum Update {
    /// The sync wrote rows or Snippets to the Cache.
    Inbox,
    /// The sync wrote the Folder list or an Unread count to the Cache.
    Folders,
    /// The server no longer has this Folder; the sync went back to the Inbox.
    FolderGone(Arc<str>),
    Bodies(Arc<str>, Fetched),
    /// The connection for bodies is up, or has gone down.
    BodyLink(bool),
    Rendered(PreviewKey, Preview),
    Status(Status),
    Backfill(Backfill),
    /// No Folder has the Trash role: the Deletes queued were taken back.
    NoTrash,
    /// An Undo found its Messages gone from Trash.
    Unrestored,
    /// SMTP accepted a Message from the Outbox.
    Sent,
    /// SMTP refused a Message from the Outbox: it is a Draft again.
    Refused,
}
