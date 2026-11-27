use crate::{
    domain::folder::Role, domain::folder_name::FolderName, domain::indexing::Indexing,
    mail::fetch::attachment_fetch::AttachmentFetched, mail::fetch::backfill::Backfill,
    mail::fetch::body_fetch::Fetched, mail::fetch::inline_fetch::InlineFetched,
    mail::status::Status, mail::watch::arrival::Arrival, render::html_job::LaidOut,
    render::preview::Preview, render::rendered_previews::PreviewKey,
};

/// Work done away from the event loop that changes what the screen shows.
pub enum Update {
    /// The sync wrote rows or Snippets to the Cache.
    Inbox,
    /// The sync wrote the Folder list or a Folder's counts to the Cache.
    Folders,
    /// The server no longer has this Folder; the sync went back to the Inbox.
    FolderGone(FolderName),
    Bodies(FolderName, Fetched),
    Attachment(AttachmentFetched),
    Inline(InlineFetched),
    FetchLink(FetchLink),
    /// New Unread Messages reached the Inbox, all in one resync or IDLE burst.
    Arrived(Vec<Arrival>),
    Rendered(PreviewKey, Preview),
    /// The HTML of a Draft's preview, laid out.
    LaidOut(LaidOut),
    Status(Status),
    Backfill(Backfill),
    Indexing(Indexing),
    /// No Folder has this role: the moves queued to it were taken back.
    NoFolder(Role),
    /// An Undo found its Messages gone from where they were moved.
    Unrestored,
    /// SMTP accepted a Message from the Outbox.
    Sent,
    /// SMTP refused a Message from the Outbox: it is a Draft again.
    Refused,
}

/// The state of an Account's fetch connection.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FetchLink {
    /// Not yet up since Postal started: bodies wait on it before showing offline.
    Starting,
    Live,
    Down,
}
