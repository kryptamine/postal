use async_imap::{Client, Session, types::Fetch, types::Flag};
use futures::TryStreamExt;

use super::{Error, connect::Transport, traced::Traced, wire_trace};
use crate::{config::Config, credential::Credential, header, inbox::Inbox, message::Message};

/// The raw `From` header rides along, for a `From` the server cannot parse.
const ITEMS: &str = "(UID FLAGS INTERNALDATE ENVELOPE BODY.PEEK[HEADER.FIELDS (FROM)])";

/// What Postal needs from a mail server, kept apart from the IMAP library.
pub trait MailServer {
    /// The newest `count` Messages of the Inbox and its Unread count.
    async fn inbox(&mut self, count: u32) -> Result<Inbox, Error>;
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
}

impl MailServer for ImapSession {
    async fn inbox(&mut self, count: u32) -> Result<Inbox, Error> {
        let exists = self.session.examine("INBOX").await?.exists;
        // async-imap cannot parse ESEARCH, so `RETURN (COUNT)` is out of reach:
        // see docs/adr/0004-unread-count-by-uid-search.md.
        let unread = self.session.uid_search("UNSEEN").await?.len();
        if exists == 0 {
            tracing::info!("the Inbox is empty");
            return Ok(Inbox {
                messages: Vec::new(),
                unread,
            });
        }

        let first = exists.saturating_sub(count.saturating_sub(1)).max(1);
        let fetches = self.session.fetch(format!("{first}:*"), ITEMS).await?;
        let messages: Vec<_> = fetches.map_ok(|f| message(&f)).try_collect().await?;
        tracing::info!("fetched {} Messages, {unread} unread", messages.len());
        Ok(Inbox { messages, unread })
    }
}

fn message(fetch: &Fetch) -> Message {
    let envelope = fetch.envelope();
    let subject = envelope
        .and_then(|e| e.subject.as_deref())
        .map(header::decode)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "(no subject)".to_owned());
    Message {
        received: fetch.internal_date().unwrap_or_default(),
        unread: !fetch.flags().any(|flag| flag == Flag::Seen),
        sender: envelope.and_then(sender).unwrap_or_else(|| raw_from(fetch)),
        subject,
    }
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
