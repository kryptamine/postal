use crate::{
    domain::folder_name::FolderName, mail::arrival::Arrival,
    mail::attachment_fetch::AttachmentFetched, mail::backfill::Backfill, mail::body_fetch::Fetched,
    mail::status::Status, render::preview::Preview, render::rendered_previews::PreviewKey,
};

/// Work done away from the event loop that changes what the screen shows.
pub enum Update {
    /// The sync wrote rows or Snippets to the Cache.
    Inbox,
    /// The sync wrote the Folder list or an Unread count to the Cache.
    Folders,
    /// The server no longer has this Folder; the sync went back to the Inbox.
    FolderGone(FolderName),
    Bodies(FolderName, Fetched),
    Attachment(AttachmentFetched),
    BodyLink(BodyLink),
    /// New Unread Messages reached the Inbox, all in one resync or IDLE burst.
    Arrived(Vec<Arrival>),
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

/// The state of the connection for bodies.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BodyLink {
    Up,
    Down,
}
