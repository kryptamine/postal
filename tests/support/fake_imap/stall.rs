//! Where the fake stops answering until a test releases it.

use std::{sync::Mutex, time::Duration};

use imap_next::imap_types::{command::CommandBody, fetch::MacroOrMessageDataItemNames};

use super::{
    Inbox,
    fetch::{body_part, whole_body},
};

/// Where the server stops answering until `FakeImap::release`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stall {
    /// Before the greeting: the client sees no byte at all.
    Greeting,
    /// After a `FETCH` or `UID FETCH` is recorded, before its answer.
    Fetch,
    /// As `Fetch`, but only for a fetch of a body part.
    Part,
    /// As `Fetch`, but only for a fetch of a whole Message, `BODY[]`.
    Whole,
    /// As `Fetch`, but only for a `FETCH` by sequence number, as the first
    /// fetch and a page of older Messages send.
    Sequence,
    /// After a `UID MOVE` is recorded, before it is made.
    Move,
}

pub(super) async fn wait_unless_stalled(inbox: &Mutex<Inbox>, stall: Stall) {
    while inbox.lock().unwrap().stall == Some(stall) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Holds a `FETCH` or a `UID MOVE` while a stall covers it.
pub(super) async fn wait_unless_command_stalled(inbox: &Mutex<Inbox>, body: &CommandBody<'static>) {
    match body {
        CommandBody::Fetch {
            macro_or_item_names,
            uid,
            ..
        } => wait_unless_fetch_stalled(inbox, macro_or_item_names, *uid).await,
        CommandBody::Move { .. } => wait_unless_stalled(inbox, Stall::Move).await,
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

    if body_part(items).is_some() {
        wait_unless_stalled(inbox, Stall::Part).await;
    }

    if whole_body(items) {
        wait_unless_stalled(inbox, Stall::Whole).await;
    }
}
