use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::{
    cache::{Cache, Changes, Reported},
    domain::folder::INBOX,
    domain::folder_name::FolderName,
    mail::folder_list,
    mail::imap::{MailServer, Mode, Pushed},
    mail::inbox_poll::InboxPoll,
    mail::outbox_flush::Sender,
    mail::queued::{send_changes, send_outbox},
    mail::sequence_numbers::SequenceNumbers,
    mail::snippets,
    mail::steering::{Asked, Steering},
    mail::sync::{self, Error},
    mail::tell::Tell,
    mail::thread_fetch,
    mail::update::Update,
};

/// How long one `IDLE` lasts before it is ended and sent again: a server
/// may drop a client idle for 30 minutes (RFC 2177), and a quiet
/// connection hides a dead one.
const PERIOD: Duration = Duration::from_secs(10 * 60);

/// Resyncs the Folder `steering` follows, then keeps its Cache live and sends
/// the Outbox until the screen stops asking; IDLE gives way to each ask (ADR
/// 0008), and to a `STATUS` of the Inbox while another Folder shows (ADR 0025).
pub async fn listen<S: MailServer>(
    mut server: S,
    cache: &Cache,
    steering: &mut Steering,
    sender: Sender<'_>,
    tell: Tell<'_, impl Fn(Update)>,
) -> Result<(), Error> {
    let Some(mut asked) = steering.take() else {
        return Ok(());
    };

    // What the last connection or run left in the Outbox.
    asked.send = true;

    // Before the resync, so it reads the flags they set and the moves they made.
    send_changes(&mut server, cache, tell.send).await?;
    let mut sequence = shown(&mut server, cache, steering, tell).await?;
    // What came during the resync and the Snippets goes first.
    let mut pushed = server.pushed();
    let mut poll = InboxPoll::new();
    loop {
        // Another Folder opened since: what the server announces is its own.
        asked.switched |= server.opening() != Some(sequence.opening());

        // Ahead of a switch, while the shown Folder is still open.
        if std::mem::take(&mut asked.changes) {
            send_changes(&mut server, cache, tell.send).await?;
            pushed.extend(server.pushed());
            continue;
        }

        if std::mem::take(&mut asked.send) {
            // A resync of the shown Folder shows a Message accepted in Sent.
            asked.switched |= send_outbox(&mut server, cache, sender, tell.send).await?;
            pushed.extend(server.pushed());
            continue;
        }

        // The switch goes first, so `STATUS` never reaches the selected Folder.
        if std::mem::take(&mut asked.switched) {
            sequence = shown(&mut server, cache, steering, tell).await?;
            pushed = server.pushed();
            continue;
        }

        if std::mem::take(&mut asked.list) {
            if !folder_list::refresh(&mut server, cache, &steering.folder, tell.send).await? {
                gone(steering, tell.send);
                asked.switched = true;
            }

            pushed.extend(server.pushed());
            continue;
        }

        if let Some(id) = asked.thread.take() {
            thread_fetch::fetch(&mut server, cache, id, tell.send).await?;
            continue;
        }

        if pushed.is_empty() {
            let held = sender.outbox.next_hold();
            let wait = poll.wait(&steering.folder, idle_for(held));
            (server, pushed) = idle(server, cache, steering, wait, tell).await?;
            let Some(more) = steering.take() else {
                return Ok(());
            };

            asked = more;
            asked.send |= sends(&asked, sender, held);
            continue;
        }

        let folder = &steering.folder;
        if let Some(why) = resync_for(cache, folder, &sequence, &pushed)? {
            tracing::info!("{why}: resyncing");
            sequence = resynced(&mut server, cache, folder, tell).await?;
            pushed = server.pushed();
            continue;
        }

        take_in(&mut server, cache, &mut sequence, pushed, tell).await?;
        pushed = server.pushed();
    }
}

/// Writes what `pushed` changed in the Folder of `sequence` to the Cache,
/// with the Snippets of new rows.
async fn take_in(
    server: &mut impl MailServer,
    cache: &Cache,
    sequence: &mut SequenceNumbers,
    pushed: Vec<Pushed>,
    tell: Tell<'_, impl Fn(Update)>,
) -> Result<(), Error> {
    let send = tell.send;
    let Some(changes) = changes(server, cache, sequence, pushed).await? else {
        return Ok(());
    };

    let folder = sequence.folder();
    cache.apply(folder, &changes)?;
    tracing::debug!(
        "IDLE: {} new, {} expunged, {} flag changes",
        changes.new.len(),
        changes.expunged.len(),
        changes.flags.len()
    );
    arrived(tell, folder, &changes);
    send(Update::Inbox);
    if !changes.new.is_empty() {
        snippets::fetch(server, cache, folder, None, || send(Update::Inbox)).await?;
    }

    Ok(())
}

/// Resyncs the Folder `steering` follows; back to the Inbox when it is gone.
async fn shown(
    server: &mut impl MailServer,
    cache: &Cache,
    steering: &mut Steering,
    tell: Tell<'_, impl Fn(Update)>,
) -> Result<SequenceNumbers, Error> {
    let folder = steering.folder.clone();
    match resynced(server, cache, &folder, tell).await {
        Err(Error::Gone(error)) if folder != *FolderName::inbox() => {
            tracing::warn!("{folder} is gone: {error}");
            cache.remove_folder(&folder)?;
            gone(steering, tell.send);
            resynced(server, cache, FolderName::inbox(), tell).await
        }
        result => result,
    }
}

/// Whether the Outbox goes next, after an IDLE that ended on `asked`; a
/// hold that ran out, `held`, is sent though another ask ended it.
fn sends(asked: &Asked, sender: Sender<'_>, held: Option<DateTime<Utc>>) -> bool {
    let due = held.is_some_and(|until| until <= Utc::now());
    due || (asked.quiet() && sender.outbox.count() > 0)
}

/// How long one `IDLE` may last: until the Message `held` is due, if sooner.
fn idle_for(held: Option<DateTime<Utc>>) -> Duration {
    let left = |until: DateTime<Utc>| (until - Utc::now()).to_std().unwrap_or_default();
    held.map_or(PERIOD, |until| left(until).min(PERIOD))
}

/// IDLEs on the shown Folder for `wait` at most, until the server
/// announces a change or the screen asks; with no `wait`, the Inbox is due
/// a look instead: gives it that and returns at once, leaving it open.
async fn idle<S: MailServer>(
    mut server: S,
    cache: &Cache,
    steering: &mut Steering,
    wait: Option<Duration>,
    tell: Tell<'_, impl Fn(Update)>,
) -> Result<(S, Vec<Pushed>), Error> {
    let Some(period) = wait else {
        poll_inbox(&mut server, cache, tell).await?;
        return Ok((server, Vec::new()));
    };

    Ok(server.idle(period, steering.asking()).await?)
}

/// Resyncs the Inbox when `STATUS` says it holds mail the Cache does not.
async fn poll_inbox(
    server: &mut impl MailServer,
    cache: &Cache,
    tell: Tell<'_, impl Fn(Update)>,
) -> Result<(), Error> {
    let inbox = FolderName::inbox();
    let Some(known) = cache.state(inbox)? else {
        return Ok(());
    };

    let next = server.uid_next_in(inbox).await?;
    if next.is_some() && next != known.uid_next {
        tracing::info!("the Inbox has new mail: resyncing it");
        resynced(server, cache, inbox, tell).await?;
    }

    Ok(())
}

/// Tells the screen of the arrivals among `changes` to `folder`.
fn arrived(tell: Tell<'_, impl Fn(Update)>, folder: &FolderName, changes: &Changes) {
    let arrived = tell.arrivals.of(folder, changes);
    if !arrived.is_empty() {
        (tell.send)(Update::Arrived(arrived));
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
    folder: &FolderName,
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
async fn resynced(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &FolderName,
    tell: Tell<'_, impl Fn(Update)>,
) -> Result<SequenceNumbers, Error> {
    // Selected, not examined, so a Mark Read can be stored: ADR 0011.
    let (examined, changes) =
        sync::resync(server, cache, folder, Mode::Select, tell.report).await?;
    let send = tell.send;
    arrived(tell, folder, &changes);
    send(Update::Inbox);
    snippets::fetch(server, cache, folder, None, || send(Update::Inbox)).await?;
    let uids = cache.uids(folder)?;
    Ok(SequenceNumbers::new(
        examined.opening,
        examined.exists,
        examined.state.uid_next,
        uids,
    ))
}

/// The rows of the Folder of `sequence` that `pushed` changes, plus the
/// Unread count; `None` before the first resync has written the Folder.
async fn changes(
    server: &mut impl MailServer,
    cache: &Cache,
    sequence: &mut SequenceNumbers,
    pushed: Vec<Pushed>,
) -> Result<Option<Changes>, Error> {
    let Some(state) = cache.state(sequence.folder())? else {
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
                flags: pushed,
            } => flags.extend(
                uid.or_else(|| sequence.uid(number))
                    .map(|uid| (uid, pushed)),
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

    // `state` is left as the resync saw it, so the next one asks for all since.
    let reported = Reported::new(state, sequence.exists(), server.unread_count().await?);
    Ok(Some(Changes::update(reported, new, expunged, flags)))
}
