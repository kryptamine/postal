use std::sync::Arc;

use crate::{
    cache::Cache,
    draft::Draft,
    folder::Role,
    imap::MailServer,
    notes::Notes,
    outbox::{Outbox, Queued},
    outgoing::Outgoing,
    role_folder,
    sign_in::SignIn,
    smtp,
    sync::Error,
    update::Update,
};

/// What sending the Outbox takes: where it is, and the Account signed in.
#[derive(Clone, Copy)]
pub struct Sender<'a> {
    pub outbox: &'a Outbox,
    pub outgoing: Outgoing<'a>,
    pub sign_in: &'a SignIn,
}

/// Sends each Message in the Outbox over SMTP, then saves its copy to Sent,
/// which Gmail does itself; true when SMTP accepted any. Only an IMAP
/// failure is returned: SMTP's failures leave a Message or give it back.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn flush(
    server: &mut impl MailServer,
    cache: &Cache,
    sender: Sender<'_>,
    send: &impl Fn(Update),
) -> Result<bool, Error> {
    let queued = match sender.outbox.queued() {
        Ok(queued) => queued,
        Err(error) => {
            tracing::error!("reading the Outbox failed: {error}");
            return Ok(false);
        }
    };

    let (mut accepted, mut unreachable) = (false, false);
    let mut sent_folder = None;
    for mut queued in queued {
        if !queued.sent {
            // A failure that is not a refusal would fail the rest the same way.
            if unreachable {
                continue;
            }

            match submit(cache, sender, &mut queued, send).await? {
                Submitted::Accepted => accepted = true,
                Submitted::Unreachable => {
                    unreachable = true;
                    continue;
                }
                Submitted::Refused | Submitted::Unread => continue,
            }
        }

        copy(server, cache, &queued, &mut sent_folder, send).await?;
    }

    Ok(accepted)
}

enum Submitted {
    Accepted,
    /// Left in the Outbox for the next sync: a network error or a `4xx`.
    Unreachable,
    /// Given back to the Drafts.
    Refused,
    /// Its files could not be read; left as they are.
    Unread,
}

/// Hands one Message to SMTP and files it by the answer.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn submit(
    cache: &Cache,
    sender: Sender<'_>,
    queued: &mut Queued,
    send: &impl Fn(Update),
) -> Result<Submitted, Error> {
    let read = std::fs::read(&queued.path).and_then(|message| {
        let notes = Notes::read(&queued.path)?;
        Ok((message, notes))
    });

    let (message, Some(notes)) = (match read {
        Ok(read) => read,
        Err(error) => {
            tracing::error!("reading {} failed: {error}", queued.path.display());
            return Ok(Submitted::Unread);
        }
    }) else {
        // Without the Draft it was, a refusal would have nothing to give back.
        tracing::error!("{} has no notes", queued.path.display());
        return Ok(Submitted::Unread);
    };

    let text = notes.draft.clone().unwrap_or_default();
    let envelope = Draft::parse(&text).and_then(|draft| sender.outgoing.envelope(&draft));
    let refusal = match envelope {
        Ok(envelope) => {
            let config = sender.outgoing.config;
            match smtp::send(config, sender.sign_in, (&envelope, &message)).await {
                Ok(()) => return accept(cache, queued, notes, send).map(|()| Submitted::Accepted),
                Err(error) if error.refused() => error.to_string(),
                Err(error) => {
                    tracing::warn!("sending {} failed: {error}", queued.path.display());
                    if let Err(error) = queued.note_failure(notes, error.to_string()) {
                        tracing::error!("keeping the error of the Send failed: {error}");
                    }

                    return Ok(Submitted::Unreachable);
                }
            }
        }
        Err(error) => error,
    };

    tracing::warn!("{} was refused: {refusal}", queued.path.display());
    let notes = Notes {
        draft: None,
        answering: notes.answering,
        error: Some(refusal),
    };

    if let Err(error) = sender.outbox.give_back(queued, (&text, notes)) {
        tracing::error!("giving a refused Message back failed: {error}");
    }

    send(Update::Refused);
    Ok(Submitted::Refused)
}

/// What follows SMTP's acceptance: the sent mark, then `\Answered` queued
/// on the Message a Reply answers.
fn accept(
    cache: &Cache,
    queued: &mut Queued,
    notes: Notes,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    tracing::info!("SMTP accepted {}", queued.path.display());
    if let Err(error) = queued.mark_sent() {
        // Unmarked, the Message is sent again should its removal fail too.
        tracing::error!("marking {} sent failed: {error}", queued.path.display());
    }

    let original = notes.answering.and_then(|answering| answering.original);
    if let Some((folder, uid)) = original {
        cache.answer(&folder, uid)?;
    }

    send(Update::Sent);
    Ok(())
}

/// Saves the copy of a Message SMTP accepted to the Folder with the Sent
/// role, then drops it from the Outbox. A refused copy is dropped too.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn copy(
    server: &mut impl MailServer,
    cache: &Cache,
    queued: &Queued,
    sent_folder: &mut Option<Arc<str>>,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    if !server.gmail() {
        if sent_folder.is_none() {
            *sent_folder = role_folder::find(server, cache, Role::Sent, send).await?;
        }

        match (sent_folder.as_deref(), std::fs::read(&queued.path)) {
            (Some(folder), Ok(message)) => match server.append(folder, &message).await {
                Err(error) if error.refusal().is_some() => {
                    tracing::warn!("saving a copy to {folder} was refused: {error}");
                }
                result => result?,
            },
            (None, _) => tracing::warn!("no Folder has the Sent role: no copy saved"),
            (_, Err(error)) => {
                tracing::error!("reading {} failed: {error}", queued.path.display());
                return Ok(());
            }
        }
    }

    if let Err(error) = queued.remove() {
        tracing::error!("removing {} failed: {error}", queued.path.display());
    }

    Ok(())
}
