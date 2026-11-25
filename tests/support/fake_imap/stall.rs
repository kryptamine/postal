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

pub(super) async fn wait_unless_stalled(inbox: &Mutex<Inbox>, stall: Stall) {
    while inbox.lock().unwrap().stall == Some(stall) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Holds a `FETCH`, a `UID MOVE` or a `UID STORE` while a stall covers it.
pub(super) async fn wait_unless_command_stalled(inbox: &Mutex<Inbox>, body: &CommandBody<'static>) {
    match body {
        CommandBody::Fetch {
            macro_or_item_names,
            uid,
            ..
        } => wait_unless_fetch_stalled(inbox, macro_or_item_names, *uid).await,
        CommandBody::Move { .. } => wait_unless_stalled(inbox, Stall::Move).await,
        CommandBody::Store { .. } => wait_unless_stalled(inbox, Stall::Store).await,
        _ => {}
    }
}

pub(super) async fn wait_unless_fetch_stalled(
    inbox: &Mutex<Inbox>,
    items: &MacroOrMessageDataItemNames<'static>,
    uid: bool,
) {
    wait_unless_stalled(inbox, Stall::Fetch).await;
    if !uid {
        wait_unless_stalled(inbox, Stall::Sequence).await;
    }

    // A body fetch asks for parts too, along with the header.
    let parts = body_parts(items);
    let header = parts.iter().any(|p| p.section == Asking::Header);
    if !parts.is_empty() && !header {
        wait_unless_stalled(inbox, Stall::Part).await;
        let past_a_snippet = |p: &PartAsked| p.partial.is_some_and(|(_, len)| len.get() > 2048);
        if parts.iter().any(past_a_snippet) {
            wait_unless_stalled(inbox, Stall::Index).await;
        }
    }

    if whole_body(items) || header {
        wait_unless_stalled(inbox, Stall::Body).await;
    }
}
