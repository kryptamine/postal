use crate::{
    cache::Cache,
    mail::flag_changes,
    mail::imap::MailServer,
    mail::move_changes,
    mail::outbox_flush::{self, Sender},
    mail::sync::Error,
    mail::thread_fetch,
    mail::update::Update,
};

/// Sends the flag changes and the moves the Cache queues, flags first, as
/// a move would leave them behind.
pub async fn send_changes(
    server: &mut impl MailServer,
    cache: &Cache,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    flag_changes::send(server, cache).await?;
    move_changes::send(server, cache, send).await
}

/// Sends the Outbox; true when SMTP accepted a Message. After one: its
/// `\Answered`, then the Thread it answers read again.
pub async fn send_outbox(
    server: &mut impl MailServer,
    cache: &Cache,
    sender: Sender<'_>,
    send: &impl Fn(Update),
) -> Result<bool, Error> {
    let accepted = outbox_flush::flush(server, cache, sender, send).await?;
    if accepted.is_empty() {
        return Ok(false);
    }

    // Before the Thread is read, or its rows lose the `\Answered` just set.
    send_changes(server, cache, send).await?;
    // Gmail files the Reply in All Mail itself; the stack shows it from there.
    for id in accepted.into_iter().flatten() {
        thread_fetch::fetch(server, cache, id, send).await?;
    }

    Ok(true)
}
