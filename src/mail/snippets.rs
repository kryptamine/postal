use std::collections::{BTreeMap, HashMap};

use crate::{
    cache::Cache, domain::folder_name::FolderName, domain::text_part::TextPart, domain::uid::Uid,
    mail::imap::MailServer, mail::one_by_one, mail::snippet, mail::sync::Error,
};

/// Messages per batch: each batch is one Cache write and one redraw.
const BATCH: usize = 50;

/// The second fetch phase: cuts a Snippet, empty when the server sends none,
/// for every Message of the examined `folder` still without one, newest first;
/// with `below`, only lower UIDs. `written` follows each batch.
pub async fn fetch(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &FolderName,
    below: Option<Uid>,
    written: impl Fn(),
) -> Result<(), Error> {
    while batch(server, cache, folder, below, &written).await? {}
    Ok(())
}

/// One batch of what `fetch` cuts; `false` when none was left to cut.
pub async fn batch(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &FolderName,
    below: Option<Uid>,
    written: impl Fn(),
) -> Result<bool, Error> {
    let pending = cache.pending_snippets(folder, BATCH, below)?;
    if pending.is_empty() {
        return Ok(false);
    }

    let snippets = cut(server, &pending).await?;
    cache.write_snippets(folder, &snippets)?;
    tracing::info!("cut {} Snippets", snippets.len());
    written();
    Ok(true)
}

/// The Snippet of each `(uid, part)` of the examined Folder, empty when the
/// server sends none.
pub async fn cut(
    server: &mut impl MailServer,
    pending: &[(Uid, TextPart)],
) -> Result<Vec<(Uid, String)>, Error> {
    let asked: Vec<_> = pending.iter().map(|(uid, part)| (*uid, part)).collect();
    let raw = parts(server, &asked, snippet::FETCHED).await?;
    Ok(pending
        .iter()
        .map(|(uid, part)| {
            let snippet = raw
                .get(uid)
                .map(|bytes| snippet::from_part(bytes, part))
                .unwrap_or_default();
            (*uid, snippet)
        })
        .collect())
}

/// The first `octets` of each `(uid, part)` of the examined Folder, as sent;
/// a UID the server did not answer for is left out.
pub async fn parts(
    server: &mut impl MailServer,
    pending: &[(Uid, &TextPart)],
    octets: u32,
) -> Result<HashMap<Uid, Vec<u8>>, Error> {
    // One FETCH asks for one section, so the batch splits by section.
    let mut by_section: BTreeMap<&str, Vec<Uid>> = BTreeMap::new();
    for (uid, part) in pending {
        by_section.entry(&part.section).or_default().push(*uid);
    }

    let mut raw = HashMap::new();
    for (section, uids) in by_section {
        let text = async |uids: &[Uid]| server.text_parts(uids, section, octets).await;
        raw.extend(one_by_one::fetch(&uids, "text part", text).await?);
    }

    Ok(raw)
}
