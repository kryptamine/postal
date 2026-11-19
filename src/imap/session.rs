use std::{collections::HashSet, time::Duration};

use async_imap::{
    Client, Session,
    extensions::idle::IdleResponse,
    imap_proto::{NameAttribute, Response, SectionPath, Status},
    types::{Fetch, Flag},
};
use futures::TryStreamExt;

use super::{
    Error, connect::Transport, new_messages, pushed::Pushed, traced::Traced, whole_bodies,
    wire_trace, xoauth2::XOAuth2,
};
use crate::{
    config::Config,
    folder::{Folder, Role},
    new_message::NewMessage,
    sign_in::SignIn,
    snippet,
    sync_state::SyncState,
};

/// The raw `From` header rides along, for a `From` the server cannot parse.
const ITEMS: &str =
    "(UID FLAGS INTERNALDATE ENVELOPE BODYSTRUCTURE BODY.PEEK[HEADER.FIELDS (FROM)])";

/// `ITEMS` with the Gmail IDs a body is cached under and a Thread is grouped
/// by, for a server that lists `X-GM-EXT-1`: anywhere else they are `BAD`.
const GMAIL_ITEMS: &str = concat!(
    "(UID FLAGS INTERNALDATE ENVELOPE X-GM-MSGID X-GM-THRID ",
    "BODYSTRUCTURE BODY.PEEK[HEADER.FIELDS (FROM)])"
);

/// A Folder as `EXAMINE` reports it.
pub struct Examined {
    pub exists: u32,
    pub state: SyncState,
}

/// What Postal needs from a mail server, kept apart from the IMAP library.
pub trait MailServer {
    /// Opens the Folder named `folder` read-only; later commands read it.
    async fn examine(&mut self, folder: &str) -> Result<Examined, Error>;

    /// Every Folder the server lists that can be opened.
    async fn folders(&mut self) -> Result<Vec<Folder>, Error>;

    /// Unread Messages in a Folder other than the examined one, by `STATUS`.
    async fn unread_in(&mut self, folder: &str) -> Result<usize, Error>;

    /// The newest `count` of the `exists` Messages, by sequence number;
    /// `progress` gets how many have come so far, now and then.
    async fn newest(
        &mut self,
        exists: u32,
        count: u32,
        progress: impl Fn(u32),
    ) -> Result<Vec<NewMessage>, Error>;

    /// Up to `count` Messages just before sequence number `before`, by
    /// sequence number; `progress` as for `newest`.
    async fn older(
        &mut self,
        before: u32,
        count: u32,
        progress: impl Fn(u32),
    ) -> Result<Vec<NewMessage>, Error>;

    /// The Messages whose UID is at least `first`.
    async fn since_uid(&mut self, first: u32) -> Result<Vec<NewMessage>, Error>;

    /// The Messages with these UIDs; a UID gone is left out.
    async fn by_uid(&mut self, uids: &[u32]) -> Result<Vec<NewMessage>, Error>;

    /// The UIDs of the examined Folder's Messages in Gmail's Thread `id`.
    async fn thread(&mut self, id: u64) -> Result<Vec<u32>, Error>;

    /// `None` when no Message has this UID any more.
    async fn sequence_number(&mut self, uid: u32) -> Result<Option<u32>, Error>;

    /// The UIDs in `first..=last` still in the Folder.
    async fn uids(&mut self, first: u32, last: u32) -> Result<HashSet<u32>, Error>;

    /// Unread per UID in `first..=last`; with `changed_since`, only for the
    /// Messages whose flags changed after that CONDSTORE mod-sequence.
    async fn unread_flags(
        &mut self,
        first: u32,
        last: u32,
        changed_since: Option<u64>,
    ) -> Result<Vec<(u32, bool)>, Error>;

    /// Unread Messages in the whole examined Folder.
    async fn unread_count(&mut self) -> Result<usize, Error>;

    /// The first `snippet::FETCHED` octets of part `section` of each of
    /// `uids`, as sent; a UID the server did not answer for is left out.
    async fn text_parts(
        &mut self,
        uids: &[u32],
        section: &str,
    ) -> Result<Vec<(u32, Vec<u8>)>, Error>;

    /// Each of `uids` whole, as `BODY[]`; a UID the server did not answer
    /// for is left out.
    async fn bodies(&mut self, uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, Error>;

    /// What the server announced during earlier commands, in order.
    fn pushed(&mut self) -> Vec<Pushed>;

    /// Waits in `IDLE` for the server to announce a change, at most `period`
    /// and no longer than `stop` takes; empty when none came.
    async fn idle(
        self,
        period: Duration,
        stop: impl Future<Output = ()>,
    ) -> Result<(Self, Vec<Pushed>), Error>
    where
        Self: Sized;
}

pub struct ImapSession {
    session: Session<Box<dyn Transport>>,
    items: &'static str,
    /// As of the last `examine`, for the body key of a server without Gmail's.
    uid_validity: u32,
    /// What the server announced during a command read by hand, not yet
    /// taken by `pushed`.
    pending: Vec<Pushed>,
    /// The Account's, to tell its own Messages.
    email: String,
}

impl ImapSession {
    #[expect(
        clippy::future_not_send,
        reason = "Postal runs on a current-thread runtime"
    )]
    pub async fn open(config: &Config, sign_in: &SignIn) -> Result<Self, Error> {
        tracing::info!("connecting to {}:{}", config.imap.host, config.imap.port);
        let mut transport = super::connect::connect(&config.imap).await?;
        if wire_trace::enabled() {
            transport = Box::new(Traced::new(transport));
        }

        let mut client = Client::new(transport);
        let greeting = client
            .read_response()
            .await
            .map_err(|e| Error::Imap(e.into()))?
            .ok_or(Error::Closed)?;
        if let Some(text) = bye(greeting.parsed()) {
            return Err(Error::Bye(text));
        }

        let mut session = match sign_in {
            SignIn::Password(password) => client.login(&config.email, password.expose()).await,
            SignIn::OAuth(tokens) => {
                let token = tokens.access().await.map_err(Error::Token)?;
                let xoauth2 = XOAuth2 {
                    email: &config.email,
                    token: token.expose(),
                    sent: false,
                };

                client.authenticate("XOAUTH2", xoauth2).await
            }
        }
        .map_err(|(e, _)| e)?;

        tracing::info!("logged in");
        let gmail = session.capabilities().await?.has_str("X-GM-EXT-1");
        Ok(Self {
            session,
            items: if gmail { GMAIL_ITEMS } else { ITEMS },
            uid_validity: 0,
            pending: Vec::new(),
            email: config.email.clone(),
        })
    }

    /// `command`, a `FETCH` of `self.items`, read by hand.
    #[expect(
        clippy::future_not_send,
        reason = "Postal runs on a current-thread runtime"
    )]
    async fn rows(
        &mut self,
        command: String,
        progress: impl Fn(u32),
    ) -> Result<Vec<NewMessage>, Error> {
        // What came before stays ahead of what comes during the fetch.
        let earlier = self.pushed();
        self.pending = earlier;
        let pushed = (&mut self.pending, progress);
        new_messages::fetch(
            &mut self.session,
            command,
            (self.uid_validity, &self.email),
            pushed,
        )
        .await
    }

    async fn uid_fetch(&mut self, uids: String, items: &str) -> Result<Vec<Fetch>, Error> {
        let fetches = self.session.uid_fetch(uids, items).await?;
        Ok(fetches.try_collect().await?)
    }
}

impl MailServer for ImapSession {
    async fn examine(&mut self, folder: &str) -> Result<Examined, Error> {
        // A plain EXAMINE suffices: a CONDSTORE server sends HIGHESTMODSEQ with
        // every EXAMINE (RFC 7162 3.1.2.1), and `(CONDSTORE)` is BAD elsewhere.
        let mailbox = self.session.examine(folder).await?;
        self.uid_validity = mailbox.uid_validity.unwrap_or_default();
        Ok(Examined {
            exists: mailbox.exists,
            state: SyncState {
                uid_validity: self.uid_validity,
                uid_next: mailbox.uid_next,
                highest_modseq: mailbox.highest_modseq,
            },
        })
    }

    async fn folders(&mut self) -> Result<Vec<Folder>, Error> {
        let names = self.session.list(Some(""), Some("*")).await?;
        let names: Vec<_> = names.try_collect().await?;
        let folders: Vec<_> = names
            .iter()
            .filter(|name| !name.attributes().contains(&NameAttribute::NoSelect))
            .map(|name| Folder::new(name.name(), role(name.attributes())))
            .collect();
        tracing::info!("listed {} Folders", folders.len());
        Ok(folders)
    }

    async fn unread_in(&mut self, folder: &str) -> Result<usize, Error> {
        let status = self.session.status(folder, "(UNSEEN)").await?;
        Ok(status
            .unseen
            .map_or(0, |n| usize::try_from(n).unwrap_or(usize::MAX)))
    }

    #[expect(
        clippy::future_not_send,
        reason = "Postal runs on a current-thread runtime"
    )]
    async fn newest(
        &mut self,
        exists: u32,
        count: u32,
        progress: impl Fn(u32),
    ) -> Result<Vec<NewMessage>, Error> {
        let first = exists.saturating_sub(count.saturating_sub(1)).max(1);
        let command = format!("FETCH {first}:* {}", self.items);
        let messages = self.rows(command, progress).await?;
        tracing::info!("fetched the newest {} Messages", messages.len());
        Ok(messages)
    }

    #[expect(
        clippy::future_not_send,
        reason = "Postal runs on a current-thread runtime"
    )]
    async fn older(
        &mut self,
        before: u32,
        count: u32,
        progress: impl Fn(u32),
    ) -> Result<Vec<NewMessage>, Error> {
        let last = before.saturating_sub(1);
        if last == 0 {
            return Ok(Vec::new());
        }

        let first = before.saturating_sub(count).max(1);
        let command = format!("FETCH {first}:{last} {}", self.items);
        let messages = self.rows(command, progress).await?;
        tracing::info!("fetched {} older Messages", messages.len());
        Ok(messages)
    }

    async fn since_uid(&mut self, first: u32) -> Result<Vec<NewMessage>, Error> {
        let command = format!("UID FETCH {first}:* {}", self.items);
        let mut messages = self.rows(command, |_| {}).await?;
        // `first:*` always matches the last Message, even below `first`.
        messages.retain(|m| m.row.uid >= first);
        tracing::info!("fetched {} new Messages", messages.len());
        Ok(messages)
    }

    async fn by_uid(&mut self, uids: &[u32]) -> Result<Vec<NewMessage>, Error> {
        let command = format!("UID FETCH {} {}", uid_set(uids), self.items);
        let messages = self.rows(command, |_| {}).await?;
        tracing::info!("fetched {} Messages by UID", messages.len());
        Ok(messages)
    }

    async fn thread(&mut self, id: u64) -> Result<Vec<u32>, Error> {
        let found = self.session.uid_search(format!("X-GM-THRID {id}")).await?;
        let mut uids: Vec<_> = found.into_iter().collect();
        uids.sort_unstable();
        Ok(uids)
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

    async fn text_parts(
        &mut self,
        uids: &[u32],
        section: &str,
    ) -> Result<Vec<(u32, Vec<u8>)>, Error> {
        let path = SectionPath::Part(
            section.split('.').filter_map(|n| n.parse().ok()).collect(),
            None,
        );
        // PEEK, or the server would mark every Message seen.
        let items = format!("(BODY.PEEK[{section}]<0.{}>)", snippet::FETCHED);
        let fetches = self.uid_fetch(uid_set(uids), &items).await?;
        Ok(fetches
            .iter()
            .filter_map(|f| Some((f.uid?, f.section(&path)?.to_vec())))
            .collect())
    }

    async fn bodies(&mut self, uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, Error> {
        whole_bodies::fetch(&mut self.session, &uid_set(uids)).await
    }

    fn pushed(&mut self) -> Vec<Pushed> {
        let mut pushed = std::mem::take(&mut self.pending);
        // async-imap drops what comes once its queue of 100 is full.
        let lost = self.session.unsolicited_responses.is_full();
        while let Ok(response) = self.session.unsolicited_responses.try_recv() {
            pushed.extend(Pushed::from_unsolicited(&response));
        }

        if lost {
            pushed.push(Pushed::Lost);
        }

        pushed
    }

    async fn idle(
        self,
        period: Duration,
        stop: impl Future<Output = ()>,
    ) -> Result<(Self, Vec<Pushed>), Error> {
        let Self {
            session,
            items,
            uid_validity,
            pending,
            email,
        } = self;
        let mut handle = session.idle();
        handle.init().await?;
        let first = {
            let (wait, _stop) = handle.wait_with_timeout(period);
            // The library's own timeout restarts on every keepalive line.
            tokio::select! {
                response = wait => response?,
                () = tokio::time::sleep(period) => IdleResponse::Timeout,
                () = stop => IdleResponse::ManualInterrupt,
            }
        };

        let mut server = Self {
            session: handle.done().await?,
            items,
            uid_validity,
            pending: Vec::new(),
            email,
        };

        let mut pushed = pending;
        if let IdleResponse::NewData(response) = &first {
            if let Some(text) = bye(response.parsed()) {
                return Err(Error::Bye(text));
            }

            pushed.extend(Pushed::from_response(response.parsed()));
        }

        // The rest of the burst came in before the server answered `DONE`.
        pushed.extend(server.pushed());
        Ok((server, pushed))
    }
}

/// The special use `LIST` gave a Folder, if one Postal lists apart.
fn role(attributes: &[NameAttribute<'_>]) -> Option<Role> {
    attributes.iter().find_map(|attribute| match attribute {
        NameAttribute::Flagged => Some(Role::Starred),
        NameAttribute::Sent => Some(Role::Sent),
        NameAttribute::Drafts => Some(Role::Drafts),
        NameAttribute::All => Some(Role::All),
        NameAttribute::Junk => Some(Role::Spam),
        NameAttribute::Trash => Some(Role::Trash),
        _ => None,
    })
}

/// The text of a `BYE`; `None` for any other response.
fn bye(response: &Response<'_>) -> Option<String> {
    match response {
        Response::Data {
            status: Status::Bye,
            outcome,
        } => Some(
            outcome
                .information
                .as_deref()
                .unwrap_or_default()
                .to_owned(),
        ),
        _ => None,
    }
}

fn uid_set(uids: &[u32]) -> String {
    uids.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn unread(fetch: &Fetch) -> bool {
    !fetch.flags().any(|flag| flag == Flag::Seen)
}
