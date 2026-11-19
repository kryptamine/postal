use std::{sync::Arc, time::Duration};

use crate::{
    cache::{Cache, Changes},
    flag_changes,
    folder::INBOX,
    folder_list,
    imap::{MailServer, Pushed},
    sequence_numbers::SequenceNumbers,
    snippets,
    status::Status,
    steering::Steering,
    sync::{self, Error},
    thread_fetch,
    update::Update,
};

/// How long one `IDLE` lasts before it is ended and sent again: a server
/// may drop a client idle for 30 minutes (RFC 2177), and a quiet
/// connection hides a dead one.
const PERIOD: Duration = Duration::from_secs(10 * 60);

/// Resyncs the Folder `steering` follows, then keeps its Cache live from the
/// server's announcements until the screen stops asking; IDLE gives way to
/// each ask (ADR 0008). `send` follows every Cache write, `report` the resyncs.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn listen<S: MailServer>(
    mut server: S,
    cache: &Cache,
    steering: &mut Steering,
    send: &impl Fn(Update),
    report: &dyn Fn(Status),
) -> Result<(), Error> {
    let Some(mut asked) = steering.take() else {
        return Ok(());
    };

    // Before the resync, so it reads the flags they set.
    flag_changes::send(&mut server, cache, None).await?;
    let mut sequence = shown(&mut server, cache, steering, send, report).await?;
    // `None` while the Folder selected on the server is not known.
    let mut selected = Some(Arc::clone(&steering.folder));
    // What came during the resync and the Snippets goes first.
    let mut pushed = server.pushed();
    loop {
        // Ahead of a switch, while `selected` is still the Folder selected.
        if std::mem::take(&mut asked.flags) {
            selected = flag_changes::send(&mut server, cache, selected.take()).await?;
            asked.switched |= selected.as_deref() != Some(&*steering.folder);
            pushed.extend(server.pushed());
            continue;
        }

        // The switch goes first, so `STATUS` never reaches the selected Folder.
        if std::mem::take(&mut asked.switched) {
            sequence = shown(&mut server, cache, steering, send, report).await?;
            selected = Some(Arc::clone(&steering.folder));
            pushed = server.pushed();
            continue;
        }

        if std::mem::take(&mut asked.list) {
            if !folder_list::refresh(&mut server, cache, &steering.folder, send).await? {
                gone(steering, send);
                asked.switched = true;
            }

            pushed.extend(server.pushed());
            continue;
        }

        if let Some(id) = asked.thread.take() {
            thread_fetch::fetch(&mut server, cache, id, send).await?;
            // All Mail is examined now, so the shown Folder is selected again.
            selected = None;
            asked.switched = true;
            continue;
        }

        if pushed.is_empty() {
            (server, pushed) = server.idle(PERIOD, steering.asking()).await?;
            let Some(more) = steering.take() else {
                return Ok(());
            };

            asked = more;
            continue;
        }

        let folder = Arc::clone(&steering.folder);
        if let Some(why) = resync_for(cache, &folder, &sequence, &pushed)? {
            tracing::info!("{why}: resyncing");
            sequence = resynced(&mut server, cache, &folder, send, report).await?;
            pushed = server.pushed();
            continue;
        }

        take_in(&mut server, cache, (&folder, &mut sequence), pushed, send).await?;
        pushed = server.pushed();
    }
}

/// Writes what `pushed` changed in `folder` to the Cache, with the Snippets
/// of new rows.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn take_in(
    server: &mut impl MailServer,
    cache: &Cache,
    (folder, sequence): (&str, &mut SequenceNumbers),
    pushed: Vec<Pushed>,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    let Some(changes) = changes(server, cache, (folder, sequence), pushed).await? else {
        return Ok(());
    };

    cache.apply(folder, &changes)?;
    tracing::debug!(
        "IDLE: {} new, {} expunged, {} flag changes",
        changes.new.len(),
        changes.expunged.len(),
        changes.flags.len()
    );
    send(Update::Inbox);
    if !changes.new.is_empty() {
        snippets::fetch(server, cache, folder, None, || send(Update::Inbox)).await?;
    }

    Ok(())
}

/// Resyncs the Folder `steering` follows, dropping what the server
/// announced of the one selected before; back to the Inbox when it is gone.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn shown(
    server: &mut impl MailServer,
    cache: &Cache,
    steering: &mut Steering,
    send: &impl Fn(Update),
    report: &dyn Fn(Status),
) -> Result<SequenceNumbers, Error> {
    server.pushed();
    let folder = Arc::clone(&steering.folder);
    match resynced(server, cache, &folder, send, report).await {
        Err(Error::Gone(error)) if *folder != *INBOX => {
            tracing::warn!("{folder} is gone: {error}");
            cache.remove_folder(&folder)?;
            gone(steering, send);
            resynced(server, cache, INBOX, send, report).await
        }
        result => result,
    }
}

/// The Folder `steering` follows is no longer on the server.
fn gone(steering: &mut Steering, send: &impl Fn(Update)) {
    let folder = std::mem::replace(&mut steering.folder, INBOX.into());
    send(Update::FolderGone(folder));
}

/// Why `pushed` cannot be read against `sequence`; `None` when it can.
fn resync_for(
    cache: &Cache,
    folder: &str,
    sequence: &SequenceNumbers,
    pushed: &[Pushed],
) -> Result<Option<&'static str>, Error> {
    if pushed.iter().any(|p| matches!(p, Pushed::Lost)) {
        return Ok(Some("IDLE announcements were dropped"));
    }

    // Rows the fetch connection added below the numbered ones take this
    // connection's own SELECT to number.
    let backfilled = matches!(
        (cache.oldest_uid(folder)?, sequence.oldest()),
        (Some(cached), Some(numbered)) if cached < numbered
    );
    Ok(backfilled.then_some("older Messages were backfilled"))
}

/// Resyncs `folder` and cuts its Snippets; numbers the cached UIDs as the
/// resync's `SELECT` did.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn resynced(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &str,
    send: &impl Fn(Update),
    report: &dyn Fn(Status),
) -> Result<SequenceNumbers, Error> {
    let examined = sync::resync(server, cache, folder, report).await?;
    send(Update::Inbox);
    snippets::fetch(server, cache, folder, None, || send(Update::Inbox)).await?;
    let uids = cache.uids(folder)?;
    Ok(SequenceNumbers::new(
        examined.exists,
        examined.state.uid_next,
        uids,
    ))
}

/// The rows of `folder` that `pushed` changes, plus the Unread count;
/// `None` before the first resync has written the Folder.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn changes(
    server: &mut impl MailServer,
    cache: &Cache,
    (folder, sequence): (&str, &mut SequenceNumbers),
    pushed: Vec<Pushed>,
) -> Result<Option<Changes>, Error> {
    let Some(state) = cache.state(folder)? else {
        return Ok(None);
    };

    let (mut grew, mut expunged, mut flags) = (false, Vec::new(), Vec::new());
    // In order: each `EXPUNGE` renumbers the Messages after it.
    for pushed in pushed {
        match pushed {
            Pushed::Exists(exists) => grew |= sequence.grew(exists),
            Pushed::Expunge(number) => expunged.extend(sequence.expunge(number)),
            Pushed::Flags {
                sequence: number,
                uid,
                unread,
            } => flags.extend(
                uid.or_else(|| sequence.uid(number))
                    .map(|uid| (uid, unread)),
            ),
            // `listen` resyncs instead of reading these.
            Pushed::Lost => {}
        }
    }

    let new = if grew {
        let first = sequence.next_uid();
        let new = server.since_uid(first).await?;
        sequence.extend(new.iter().map(|m| m.row.uid));
        // The fetch saw these flags later than any announcement did.
        flags.retain(|&(uid, _)| uid < first);
        new
    } else {
        Vec::new()
    };

    Ok(Some(Changes {
        // Left as the resync saw it, so the next one asks for all since.
        state,
        unread: server.unread_count().await?,
        replace: false,
        new,
        expunged,
        flags,
    }))
}
