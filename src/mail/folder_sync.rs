use crate::{
    cache::Cache,
    domain::folder_name::FolderName,
    mail::changes::queued::{send_changes, send_outbox},
    mail::fetch::body_fetch,
    mail::imap::{ImapSession, Mode},
    mail::outbox::outbox_flush::Sender,
    mail::passes::body_index,
    mail::passes::snippets,
    mail::sync::{self, Error},
    mail::update::Update,
};

/// How many of the newest new Messages get their bodies fetched and
/// indexed; the TUI's passes fetch the rest.
const BODIES: usize = 50;

/// Sends what the TUI left queued, then brings `folder` up to the server's
/// on one connection, as the TUI's sync does on connecting; how many new
/// Messages it brought.
pub async fn sync(cache: &Cache, sender: Sender<'_>, folder: &FolderName) -> Result<usize, Error> {
    // No screen is there to tell.
    let unseen = |_: Update| {};
    let mut server = ImapSession::connect(sender.outgoing.config, sender.sign_in).await?;
    send_changes(&mut server, cache, &unseen).await?;
    send_outbox(&mut server, cache, sender, &unseen).await?;
    let (_, changes) = sync::resync(&mut server, cache, folder, Mode::Examine, &|_| {}).await?;
    snippets::fetch(&mut server, cache, folder, None, || {}).await?;
    let mut newest: Vec<_> = changes.new.iter().map(|m| m.row.uid).collect();
    newest.sort_unstable_by(|a, b| b.cmp(a));
    newest.truncate(BODIES);
    let bodied = newest.len();
    if bodied > 0 {
        body_fetch::fetch_each(&mut server, cache, folder, newest, &unseen).await?;
    }

    // The rows written last are the newest the index lacks, so it takes them first.
    body_index::index_newest(&mut server, cache, bodied).await?;
    Ok(changes.new.len())
}
