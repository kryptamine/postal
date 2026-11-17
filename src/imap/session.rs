use async_imap::{Client, Session, types::Fetch, types::Flag};
use futures::TryStreamExt;

use super::{Error, connect::Transport};
use crate::{config::Config, header, message::Message};

/// What Postal needs from a mail server, kept apart from the IMAP library.
pub trait MailServer {
    /// The newest `count` Messages of the Inbox, in server order.
    async fn newest(&mut self, count: u32) -> Result<Vec<Message>, Error>;
}

pub struct ImapSession {
    session: Session<Box<dyn Transport>>,
}

impl ImapSession {
    pub async fn open(config: &Config, password: &str) -> Result<Self, Error> {
        let mut client = Client::new(super::connect::connect(&config.imap).await?);
        client
            .read_response()
            .await
            .map_err(|e| Error::Imap(e.into()))?
            .ok_or(Error::Closed)?;
        let session = client
            .login(&config.email, password)
            .await
            .map_err(|(e, _)| e)?;
        Ok(Self { session })
    }

    pub async fn logout(mut self) -> Result<(), Error> {
        Ok(self.session.logout().await?)
    }
}

impl MailServer for ImapSession {
    async fn newest(&mut self, count: u32) -> Result<Vec<Message>, Error> {
        let exists = self.session.examine("INBOX").await?.exists;
        if exists == 0 {
            return Ok(Vec::new());
        }

        let first = exists.saturating_sub(count.saturating_sub(1)).max(1);
        let fetches = self
            .session
            .fetch(format!("{first}:*"), "(UID FLAGS INTERNALDATE ENVELOPE)")
            .await?;
        Ok(fetches.map_ok(|f| message(&f)).try_collect().await?)
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
        sender: envelope.map(sender).unwrap_or_default(),
        subject,
    }
}

/// The `From` display name, else the bare address.
fn sender(envelope: &async_imap::imap_proto::Envelope<'_>) -> String {
    let Some(from) = envelope.from.as_ref().and_then(|list| list.first()) else {
        return String::new();
    };

    if let Some(name) = from.name.as_deref().map(header::decode)
        && !name.trim().is_empty()
    {
        return name;
    }

    let mut address =
        String::from_utf8_lossy(from.mailbox.as_deref().unwrap_or_default()).into_owned();
    if let Some(host) = from.host.as_deref() {
        address.push('@');
        address.push_str(&String::from_utf8_lossy(host));
    }

    address
}
