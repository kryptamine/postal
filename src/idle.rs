use std::time::Duration;

use crate::{
    cache::{Cache, Changes},
    imap::{Examined, MailServer, Pushed},
    sequence_numbers::SequenceNumbers,
    snippets,
    sync::Error,
};

/// How long one `IDLE` lasts before it is ended and sent again: a server
/// may drop a client idle for 30 minutes (RFC 2177), and a quiet
/// connection hides a dead one.
const PERIOD: Duration = Duration::from_secs(10 * 60);

/// Keeps the Cache live from the server's announcements, forever:
/// `examined` is the resync's `EXAMINE`. `written` follows every Cache write.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn listen<S: MailServer>(
    mut server: S,
    cache: &Cache,
    examined: &Examined,
    written: impl Fn(),
) -> Result<(), Error> {
    let uids = cache.uids()?;
    let mut sequence = SequenceNumbers::new(examined.exists, examined.state.uid_next, uids);
    // What came during the resync and the Snippets goes first.
    let mut pushed = server.pushed();
    loop {
        if pushed.is_empty() {
            (server, pushed) = server.idle(PERIOD).await?;
            continue;
        }

        let Some(changes) = changes(&mut server, cache, &mut sequence, pushed).await? else {
            pushed = server.pushed();
            continue;
        };

        cache.apply(&changes)?;
        tracing::debug!(
            "IDLE: {} new, {} expunged, {} flag changes",
            changes.new.len(),
            changes.expunged.len(),
            changes.flags.len()
        );
        written();
        if !changes.new.is_empty() {
            snippets::fetch(&mut server, cache, &written).await?;
        }

        pushed = server.pushed();
    }
}

/// The rows `pushed` changes, plus the Unread count; `None` before the
/// first resync has written the Inbox.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn changes(
    server: &mut impl MailServer,
    cache: &Cache,
    sequence: &mut SequenceNumbers,
    pushed: Vec<Pushed>,
) -> Result<Option<Changes>, Error> {
    let Some(state) = cache.state()? else {
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
