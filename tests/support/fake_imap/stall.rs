//! Where the fake stops answering until a test releases it.

use std::{sync::Mutex, time::Duration};

use imap_next::imap_types::{command::CommandBody, fetch::MacroOrMessageDataItemNames};

use super::{
    Inbox,
    fetch::{Asking, PartAsked, body_parts, whole_body},
};

/// Where the server stops answering until `FakeImap::release`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stall {
    /// Before the greeting: the client sees no byte at all.
    Greeting,
    /// After a `FETCH` or `UID FETCH` is recorded, before its answer.
    Fetch,
    /// As `Fetch`, but only for a fetch of body parts alone, as a Snippet's.
    Part,
    /// As `Part`, but only for a fetch of more than a Snippet's octets, as
    /// the body index's.
    Index,
    /// As `Fetch`, but only for a fetch of a Message's body: `BODY[]`, or
    /// the header asked with its text parts.
    Body,
    /// As `Fetch`, but only for a `FETCH` by sequence number, as the first
    /// fetch and a page of older Messages send.
    Sequence,
    /// After a `UID MOVE` is recorded, before it is made.
    Move,
    /// After a `UID STORE` is recorded, before it is made.
    Store,
}

pub(super) async fn wait_unless_stalled(inbox: &Mutex<Inbox>, stall: Stall, cut: Option<u64>) {
    while {
        let inbox = inbox.lock().unwrap();
        inbox.stall == Some(stall) && cut.is_none_or(|c| inbox.cut == c)
    } {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Holds a `FETCH`, a `UID MOVE` or a `UID STORE` while a stall covers it.
/// Returns false when `fail` cut the connection while it waited.
pub(super) async fn wait_unless_command_stalled(
    inbox: &Mutex<Inbox>,
    body: &CommandBody<'static>,
    cut: u64,
) -> bool {
    match body {
        CommandBody::Fetch {
            macro_or_item_names,
            uid,
            ..
        } => wait_unless_fetch_stalled(inbox, macro_or_item_names, *uid, cut).await,
        CommandBody::Move { .. } => wait_unless_stalled(inbox, Stall::Move, Some(cut)).await,
        CommandBody::Store { .. } => wait_unless_stalled(inbox, Stall::Store, Some(cut)).await,
        _ => {}
    }

    inbox.lock().unwrap().cut == cut
}

pub(super) async fn wait_unless_fetch_stalled(
    inbox: &Mutex<Inbox>,
    items: &MacroOrMessageDataItemNames<'static>,
    uid: bool,
    cut: u64,
) {
    wait_unless_stalled(inbox, Stall::Fetch, Some(cut)).await;
    if !uid {
        wait_unless_stalled(inbox, Stall::Sequence, Some(cut)).await;
    }

    // A body fetch asks for parts too, along with the header.
    let parts = body_parts(items);
    let header = parts.iter().any(|p| p.section == Asking::Header);
    if !parts.is_empty() && !header {
        wait_unless_stalled(inbox, Stall::Part, Some(cut)).await;
        let past_a_snippet = |p: &PartAsked| p.partial.is_some_and(|(_, len)| len.get() > 2048);
        if parts.iter().any(past_a_snippet) {
            wait_unless_stalled(inbox, Stall::Index, Some(cut)).await;
        }
    }

    if whole_body(items) || header {
        wait_unless_stalled(inbox, Stall::Body, Some(cut)).await;
    }
}
