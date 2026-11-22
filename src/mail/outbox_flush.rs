use chrono::Utc;
use mail_parser::MessageParser;

use crate::{
    account::sign_in::SignIn,
    cache::Cache,
    compose::draft::Draft,
    compose::notes::Notes,
    compose::outbox::{Outbox, Queued},
    compose::outgoing::Outgoing,
    domain::correspondence::{self, Correspondence, Person},
    domain::folder::Role,
    domain::folder_name::FolderName,
    domain::message_ref::MessageRef,
    domain::thread_id::ThreadId,
    mail::imap::MailServer,
    mail::role_folder,
    mail::smtp,
    mail::sync::Error,
    mail::update::Update,
};

/// What sending the Outbox takes: where it is, and the Account signed in.
#[derive(Clone, Copy)]
pub struct Sender<'a> {
    pub outbox: &'a Outbox,
    pub outgoing: Outgoing<'a>,
    pub sign_in: &'a SignIn,
    /// The first Account, which sends the Messages naming none.
    pub first: bool,
}

impl Sender<'_> {
    /// Whether `queued` goes out from this Account; one whose notes do not
    /// read is left to the first.
    fn sends(self, queued: &Queued) -> bool {
        let notes = Notes::read(&queued.path).ok().flatten();
        let email = &self.outgoing.config.email;
        notes
            .and_then(|notes| notes.account)
            .map_or(self.first, |account| account.eq_ignore_ascii_case(email))
    }
}

/// Sends each Message in the Outbox over SMTP, then saves its copy to Sent,
/// which Gmail does itself; one entry per Message SMTP accepted, naming the
/// Gmail Thread it answers. Only an IMAP failure is returned: SMTP's
/// failures leave a Message or give it back.
pub async fn flush(
    server: &mut impl MailServer,
    cache: &Cache,
    sender: Sender<'_>,
    send: &impl Fn(Update),
) -> Result<Vec<Option<ThreadId>>, Error> {
    let queued = match sender.outbox.queued() {
        Ok(queued) => queued,
        Err(error) => {
            tracing::error!("reading the Outbox failed: {error}");
            return Ok(Vec::new());
        }
    };

    let (mut accepted, mut unreachable) = (Vec::new(), false);
    let mut sent_folder = None;
    for mut queued in queued.into_iter().filter(|queued| sender.sends(queued)) {
        if !queued.sent {
            // A failure that is not a refusal would fail the rest the same way.
            if unreachable {
                continue;
            }

            match submit(cache, sender, &mut queued, send).await? {
                Submitted::Accepted(thread) => accepted.push(thread),
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
    /// With the Gmail Thread the Message answers.
    Accepted(Option<ThreadId>),
    /// Left in the Outbox for the next sync: a network error or a `4xx`.
    Unreachable,
    /// Given back to the Drafts.
    Refused,
    /// Its files could not be read; left as they are.
    Unread,
}

/// Hands one Message to SMTP and files it by the answer.
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
    let parsed =
        Draft::parse(&text).and_then(|draft| Ok((sender.outgoing.envelope(&draft)?, draft)));
    let refusal = match parsed {
        Ok((envelope, draft)) => {
            let config = sender.outgoing.config;
            match smtp::send(config, sender.sign_in, &envelope, &message).await {
                Ok(()) => {
                    let written = correspondence(&draft, &message);
                    let thread = notes.answering.as_ref().and_then(|a| a.thread);
                    return accept(cache, queued, notes, written.as_ref(), send)
                        .map(|()| Submitted::Accepted(thread));
                }
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
        account: notes.account,
        answering: notes.answering,
        error: Some(refusal),
    };

    if let Err(error) = sender.outbox.give_back(queued, &text, &notes) {
        tracing::error!("giving a refused Message back failed: {error}");
    }

    send(Update::Refused);
    Ok(Submitted::Refused)
}

/// Everyone `draft` went out to, under the `Message-ID` Postal gave
/// `message`; `None` without one, as it could not be told from others.
fn correspondence(draft: &Draft, message: &[u8]) -> Option<Correspondence> {
    let headers = MessageParser::default().parse_headers(message)?;
    let people = draft.to.iter().chain(draft.cc.iter());
    Some(Correspondence {
        message: correspondence::key(headers.message_id()?),
        at: Utc::now(),
        from: Vec::new(),
        to: people
            .map(|to| Person::new(to.email.as_ref(), to.name.clone()))
            .collect(),
        sent: true,
    })
}

/// What follows SMTP's acceptance: the sent mark, Correspondents learnt
/// from `written`, then `\Answered` queued on the Message a Reply answers.
fn accept(
    cache: &Cache,
    queued: &mut Queued,
    notes: Notes,
    written: Option<&Correspondence>,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    tracing::info!("SMTP accepted {}", queued.path.display());
    if let Err(error) = queued.mark_sent() {
        // Unmarked, the Message is sent again should its removal fail too.
        tracing::error!("marking {} sent failed: {error}", queued.path.display());
    }

    if let Some(written) = written {
        cache.sent(written)?;
    }

    let original = notes.answering.and_then(|answering| answering.original);
    if let Some(MessageRef { folder, uid }) = original {
        cache.answer(&folder, uid)?;
    }

    send(Update::Sent);
    Ok(())
}

/// Saves the copy of a Message SMTP accepted to the Folder with the Sent
/// role, then drops it from the Outbox. A refused copy is dropped too.
async fn copy(
    server: &mut impl MailServer,
    cache: &Cache,
    queued: &Queued,
    sent_folder: &mut Option<FolderName>,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    if !server.gmail() {
        if sent_folder.is_none() {
            *sent_folder = role_folder::find(server, cache, Role::Sent, send).await?;
        }

        match (sent_folder.as_ref(), std::fs::read(&queued.path)) {
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
