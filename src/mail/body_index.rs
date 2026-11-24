use crate::{
    cache::{Cache, Indexed},
    domain::html_text,
    domain::text_part::TextPart,
    mail::body_fetch::open,
    mail::imap::MailServer,
    mail::snippet,
    mail::snippets,
    mail::sync::Error,
    mail::update::Update,
};

/// Messages per batch: a Preview asked for meanwhile waits on one batch.
const BATCH: usize = 20;

/// The octets of a body indexed; what follows is mostly quoted history.
const OCTETS: u32 = 32 * 1024;

/// Indexes the bodies of one batch of the newest Messages whose body the
/// index lacks, a Folder at a time, cutting their Snippets too when
/// missing. `false` when none was left, or the server would not give them.
pub async fn batch(
    server: &mut impl MailServer,
    cache: &Cache,
    send: &impl Fn(Update),
) -> Result<bool, Error> {
    let Some((folder, pending)) = cache.unindexed_bodies(BATCH)? else {
        return Ok(false);
    };

    if !open(server, &folder).await? {
        return Ok(false);
    }

    let parts: Vec<_> = pending
        .iter()
        .filter_map(|p| p.part.as_ref().map(|part| (p.uid, part)))
        .collect();
    let raw = match snippets::parts(server, &parts, OCTETS).await {
        Err(Error::Imap(error)) if error.refusal().is_some() => {
            tracing::warn!("indexing bodies of {folder} was refused: {error}");
            return Ok(false);
        }
        raw => raw?,
    };

    let indexed: Vec<_> = pending
        .iter()
        .map(|p| {
            let fetched = p.part.as_ref().zip(raw.get(&p.uid));
            Indexed {
                uid: p.uid,
                text: fetched.map(|(part, bytes)| text(bytes, part)),
                snippet: fetched
                    .filter(|_| p.snippet_missing)
                    .map(|(part, bytes)| snippet::from_part(bytes, part)),
            }
        })
        .collect();
    cache.index_bodies(&folder, &indexed)?;
    tracing::info!("indexed {} bodies of {folder}", indexed.len());
    send(Update::Inbox);
    Ok(true)
}

/// The words of a fetched part, markup stripped.
fn text(raw: &[u8], part: &TextPart) -> String {
    let text = part.decode(raw);
    if part.html {
        html_text::strip(&text)
    } else {
        text
    }
}
