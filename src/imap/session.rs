use std::collections::HashSet;

use async_imap::{Client, Session, types::Fetch, types::Flag};
use futures::TryStreamExt;

use super::{Error, connect::Transport, traced::Traced, wire_trace};
use crate::{
    config::Config, credential::Credential, header, message::Message, sync_state::SyncState,
};

/// The raw `From` header rides along, for a `From` the server cannot parse.
const ITEMS: &str = "(UID FLAGS INTERNALDATE ENVELOPE BODY.PEEK[HEADER.FIELDS (FROM)])";

/// The Inbox as `EXAMINE` reports it.
pub struct Examined {
    pub exists: u32,
    pub state: SyncState,
}

/// What Postal needs from a mail server, kept apart from the IMAP library.
pub trait MailServer {
    async fn examine(&mut self) -> Result<Examined, Error>;

    /// The newest `count` of the `exists` Messages, by sequence number.
    async fn newest(&mut self, exists: u32, count: u32) -> Result<Vec<Message>, Error>;

    /// The Messages whose UID is at least `first`.
    async fn since_uid(&mut self, first: u32) -> Result<Vec<Message>, Error>;

    /// `None` when no Message has this UID any more.
    async fn sequence_number(&mut self, uid: u32) -> Result<Option<u32>, Error>;

    /// The UIDs in `first..=last` still in the Inbox.
    async fn uids(&mut self, first: u32, last: u32) -> Result<HashSet<u32>, Error>;

    /// Unread per UID in `first..=last`; with `changed_since`, only for the
    /// Messages whose flags changed after that CONDSTORE mod-sequence.
    async fn unread_flags(
        &mut self,
        first: u32,
        last: u32,
        changed_since: Option<u64>,
    ) -> Result<Vec<(u32, bool)>, Error>;

    /// Unread Messages in the whole Inbox.
    async fn unread_count(&mut self) -> Result<usize, Error>;
}

pub struct ImapSession {
    session: Session<Box<dyn Transport>>,
}

impl ImapSession {
    pub async fn open(config: &Config, password: &Credential) -> Result<Self, Error> {
        tracing::info!("connecting to {}:{}", config.imap.host, config.imap.port);
        let mut transport = super::connect::connect(&config.imap).await?;
        if wire_trace::enabled() {
            transport = Box::new(Traced::new(transport));
        }

        let mut client = Client::new(transport);
        client
            .read_response()
            .await
            .map_err(|e| Error::Imap(e.into()))?
            .ok_or(Error::Closed)?;
        let session = client
            .login(&config.email, password.expose())
            .await
            .map_err(|(e, _)| e)?;
        tracing::info!("logged in");
        Ok(Self { session })
    }

    pub async fn logout(mut self) -> Result<(), Error> {
        Ok(self.session.logout().await?)
    }

    async fn uid_fetch(&mut self, uids: String, items: &str) -> Result<Vec<Fetch>, Error> {
        let fetches = self.session.uid_fetch(uids, items).await?;
        Ok(fetches.try_collect().await?)
    }
}

impl MailServer for ImapSession {
    async fn examine(&mut self) -> Result<Examined, Error> {
        // A plain EXAMINE suffices: a CONDSTORE server sends HIGHESTMODSEQ with
        // every EXAMINE (RFC 7162 3.1.2.1), and `(CONDSTORE)` is BAD elsewhere.
        let mailbox = self.session.examine("INBOX").await?;
        Ok(Examined {
            exists: mailbox.exists,
            state: SyncState {
                uid_validity: mailbox.uid_validity.unwrap_or_default(),
                uid_next: mailbox.uid_next,
                highest_modseq: mailbox.highest_modseq,
            },
        })
    }

    async fn newest(&mut self, exists: u32, count: u32) -> Result<Vec<Message>, Error> {
        let first = exists.saturating_sub(count.saturating_sub(1)).max(1);
        let fetches = self.session.fetch(format!("{first}:*"), ITEMS).await?;
        let messages: Vec<_> = fetches
            .try_filter_map(|f| futures::future::ok(message(&f)))
            .try_collect()
            .await?;
        tracing::info!("fetched the newest {} Messages", messages.len());
        Ok(messages)
    }

    async fn since_uid(&mut self, first: u32) -> Result<Vec<Message>, Error> {
        let fetches = self.uid_fetch(format!("{first}:*"), ITEMS).await?;
        // `first:*` always matches the last Message, even below `first`.
        let messages: Vec<_> = fetches
            .iter()
            .filter_map(message)
            .filter(|m| m.uid >= first)
            .collect();
        tracing::info!("fetched {} new Messages", messages.len());
        Ok(messages)
    }

    async fn sequence_number(&mut self, uid: u32) -> Result<Option<u32>, Error> {
        let fetches = self.uid_fetch(uid.to_string(), "UID").await?;
        Ok(fetches
            .iter()
            .find(|f| f.uid == Some(uid))
            .map(|f| f.message))
    }

    async fn uids(&mut self, first: u32, last: u32) -> Result<HashSet<u32>, Error> {
        // `RETURN (ALL)` is out of reach, as for the Unread count:
        // see docs/adr/0004-unread-count-by-uid-search.md.
        Ok(self
            .session
            .uid_search(format!("UID {first}:{last}"))
            .await?)
    }

    async fn unread_flags(
        &mut self,
        first: u32,
        last: u32,
        changed_since: Option<u64>,
    ) -> Result<Vec<(u32, bool)>, Error> {
        let items = changed_since.map_or_else(
            || "(UID FLAGS)".to_owned(),
            |modseq| format!("(UID FLAGS) (CHANGEDSINCE {modseq})"),
        );
        let fetches = self.uid_fetch(format!("{first}:{last}"), &items).await?;
        let flags: Vec<_> = fetches
            .iter()
            .filter_map(|f| Some((f.uid?, unread(f))))
            .collect();
        tracing::info!("fetched flags of {} Messages", flags.len());
        Ok(flags)
    }

    async fn unread_count(&mut self) -> Result<usize, Error> {
        // async-imap cannot parse ESEARCH, so `RETURN (COUNT)` is out of reach:
        // see docs/adr/0004-unread-count-by-uid-search.md.
        Ok(self.session.uid_search("UNSEEN").await?.len())
    }
}

fn unread(fetch: &Fetch) -> bool {
    !fetch.flags().any(|flag| flag == Flag::Seen)
}

/// `None` when the server left out the UID.
fn message(fetch: &Fetch) -> Option<Message> {
    let envelope = fetch.envelope();
    let subject = envelope
        .and_then(|e| e.subject.as_deref())
        .map(header::decode)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "(no subject)".to_owned());
    Some(Message {
        uid: fetch.uid?,
        received: fetch.internal_date().unwrap_or_default(),
        unread: unread(fetch),
        sender: envelope.and_then(sender).unwrap_or_else(|| raw_from(fetch)),
        subject,
    })
}

/// The `From` display name, else the full address; `None` when the server
/// found no address in the header.
fn sender(envelope: &async_imap::imap_proto::Envelope<'_>) -> Option<String> {
    let from = envelope.from.as_ref()?.first()?;
    let mailbox = from.mailbox.as_deref().filter(|m| !m.is_empty())?;
    let host = from.host.as_deref().filter(|h| !h.is_empty())?;
    if let Some(name) = from.name.as_deref().map(header::decode)
        && !name.trim().is_empty()
    {
        return Some(name);
    }

    Some(format!(
        "{}@{}",
        String::from_utf8_lossy(mailbox),
        String::from_utf8_lossy(host)
    ))
}

/// The `From` header's value as sent, decoded to one line.
fn raw_from(fetch: &Fetch) -> String {
    let header = fetch.header().unwrap_or_default();
    let value = header
        .iter()
        .position(|&b| b == b':')
        .map_or(&[][..], |colon| &header[colon + 1..]);
    header::decode(value).trim().to_owned()
}
