use std::collections::{BTreeMap, HashMap};

use crate::{cache::Cache, imap::MailServer, snippet, sync::Error};

/// Messages per batch: each batch is one Cache write and one redraw.
const BATCH: usize = 50;

/// The second fetch phase: cuts a Snippet for every cached Message still
/// without one, newest first, calling `written` after each batch; with
/// `below`, only those with a lower UID. Every pending Message gets one,
/// empty when the server sends nothing for it.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn fetch(
    server: &mut impl MailServer,
    cache: &Cache,
    below: Option<u32>,
    written: impl Fn(),
) -> Result<(), Error> {
    while batch(server, cache, below, &written).await? {}
    Ok(())
}

/// One batch of what `fetch` cuts; `false` when none was left to cut.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn batch(
    server: &mut impl MailServer,
    cache: &Cache,
    below: Option<u32>,
    written: impl Fn(),
) -> Result<bool, Error> {
    let pending = cache.pending_snippets(BATCH, below)?;
    if pending.is_empty() {
        return Ok(false);
    }

    // One FETCH asks for one section, so the batch splits by section.
    let mut by_section: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    for (uid, part) in &pending {
        by_section.entry(&part.section).or_default().push(*uid);
    }

    let mut raw = HashMap::new();
    for (section, uids) in by_section {
        raw.extend(server.text_parts(&uids, section).await?);
    }

    let snippets: Vec<_> = pending
        .iter()
        .map(|(uid, part)| {
            let snippet = raw
                .get(uid)
                .map(|bytes| snippet::from_part(bytes, part))
                .unwrap_or_default();
            (*uid, snippet)
        })
        .collect();
    cache.write_snippets(&snippets)?;
    tracing::info!("cut {} Snippets", snippets.len());
    written();
    Ok(true)
}
