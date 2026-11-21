use std::{collections::HashSet, time::Duration};

use async_imap::{
    Client, Session,
    extensions::idle::IdleResponse,
    imap_proto::{NameAttribute, Response, SectionPath, Status},
    types::{Fetch, Flag},
};
use futures::TryStreamExt;

use super::{
    Error, Moved,
    connect::Transport,
    login, moved_uids, named_roles, new_messages,
    opening::{Mode, Opening},
    origin::Origin,
    pushed::Pushed,
    tagged,
    traced::Traced,
    whole_bodies, wire_trace,
};
use crate::{
    account::config::Config,
    account::sign_in::SignIn,
    domain::correspondence::Correspondence,
    domain::flag,
    domain::folder::{Folder, INBOX, Role},
    domain::folder_name::FolderName,
    domain::message_flags::MessageFlags,
    domain::new_message::NewMessage,
    domain::sync_state::SyncState,
    domain::thread_id::ThreadId,
    domain::uid::Uid,
    mail::snippet,
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

/// What a Correspondent is learnt from, for the pass over Sent.
const ENVELOPE_ITEMS: &str = "(UID INTERNALDATE ENVELOPE)";

/// How often a server without `IDLE` is asked what changed.
const POLL: Duration = Duration::from_secs(60);

/// A Folder as `EXAMINE` or `SELECT` reports it.
pub struct Examined {
    pub exists: u32,
    pub state: SyncState,
    pub opening: Opening,
}

/// What Postal needs from a mail server, kept apart from the IMAP library.
pub trait MailServer {
    /// Opens `folder` as `mode`, unless the open Folder already covers it;
    /// later commands read it. An open on the wire is as `reopen`'s.
    async fn open(&mut self, folder: &FolderName, mode: Mode) -> Result<(), Error>;

    /// Opens `folder` as `mode` on the wire, for this moment's counts. Drops
    /// what the server announced of the Folder open before.
    async fn reopen(&mut self, folder: &FolderName, mode: Mode) -> Result<Examined, Error>;

    /// The last open; `None` before one, and after one failed.
    fn opening(&self) -> Option<&Opening>;

    /// Adds `flag` to `uids` of the selected Folder, or removes it.
    async fn store(&mut self, uids: &[Uid], flag: flag::Flag, added: bool) -> Result<(), Error>;

    /// Moves `uids` of the selected Folder to the Folder named `to`, and
    /// returns each move the server named.
    async fn move_to(&mut self, uids: &[Uid], to: &FolderName) -> Result<Vec<Moved>, Error>;

    /// Saves `message` to the Folder named `folder`, flagged `\Seen`.
    async fn append(&mut self, folder: &FolderName, message: &[u8]) -> Result<(), Error>;

    /// Gmail, which keeps a copy of each Message sent over SMTP itself.
    fn gmail(&self) -> bool;

    /// Every Folder the server lists that can be opened.
    async fn folders(&mut self) -> Result<Vec<Folder<()>>, Error>;

    /// Unread Messages in a Folder other than the examined one, by `STATUS`.
    async fn unread_in(&mut self, folder: &FolderName) -> Result<usize, Error>;

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
    async fn since_uid(&mut self, first: Uid) -> Result<Vec<NewMessage>, Error>;

    /// As `newest`, with nothing but each Message's UID and addresses read.
    async fn newest_envelopes(
        &mut self,
        exists: u32,
        count: u32,
    ) -> Result<Vec<(Uid, Correspondence)>, Error>;

    /// As `since_uid`, with nothing but each Message's UID and addresses read.
    async fn envelopes_since(&mut self, first: Uid) -> Result<Vec<(Uid, Correspondence)>, Error>;

    /// The Messages with these UIDs; a UID gone is left out.
    async fn by_uid(&mut self, uids: &[Uid]) -> Result<Vec<NewMessage>, Error>;

    /// The UIDs of the examined Folder's Messages in Gmail's Thread `id`.
    async fn thread(&mut self, id: ThreadId) -> Result<Vec<Uid>, Error>;

    /// `None` when no Message has this UID any more.
    async fn sequence_number(&mut self, uid: Uid) -> Result<Option<u32>, Error>;

    /// The UIDs in `first..=last` still in the Folder.
    async fn uids(&mut self, first: Uid, last: Uid) -> Result<HashSet<Uid>, Error>;

    /// The flags per UID in `first..=last`; with `changed_since`, only for the
    /// Messages whose flags changed after that CONDSTORE mod-sequence.
    async fn flags(
        &mut self,
        first: Uid,
        last: Uid,
        changed_since: Option<u64>,
    ) -> Result<Vec<(Uid, MessageFlags)>, Error>;

    /// Unread Messages in the whole examined Folder.
    async fn unread_count(&mut self) -> Result<usize, Error>;

    /// The first `snippet::FETCHED` octets of part `section` of each of
    /// `uids`, as sent; a UID the server did not answer for is left out.
    async fn text_parts(
        &mut self,
        uids: &[Uid],
        section: &str,
    ) -> Result<Vec<(Uid, Vec<u8>)>, Error>;

    /// Each of `uids` whole, as `BODY[]`; a UID the server did not answer
    /// for is left out.
    async fn bodies(&mut self, uids: &[Uid]) -> Result<Vec<(Uid, Vec<u8>)>, Error>;

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
    /// As of the last open, for the body key of a server without Gmail's.
    uid_validity: u32,
    /// A failed `SELECT` or `EXAMINE` leaves none open (RFC 3501 6.3.1).
    opening: Option<Opening>,
    /// Opens made on the wire, to tell one open of a Folder from the next.
    opens: u64,
    /// What the server announced during a command read by hand, not yet
    /// taken by `pushed`.
    pending: Vec<Pushed>,
    /// The Account's, to tell its own Messages.
    email: String,
    listed: Listed,
    gmail: bool,
}

/// What the server lists of `MOVE` (RFC 6851), `UIDPLUS` (RFC 4315) and
/// `IDLE` (RFC 2177); without `IDLE`, `idle` polls by `NOOP`.
#[derive(Clone, Copy)]
struct Listed {
    moves: bool,
    uidplus: bool,
    idle: bool,
}

impl ImapSession {
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

        let mut session = login::sign_in(client, config, sign_in).await?;

        tracing::info!("logged in");
        let capabilities = session.capabilities().await?;
        let gmail = capabilities.has_str("X-GM-EXT-1");
        let listed = Listed {
            moves: capabilities.has_str("MOVE"),
            uidplus: capabilities.has_str("UIDPLUS"),
            idle: capabilities.has_str("IDLE"),
        };
        Ok(Self {
            session,
            items: if gmail { GMAIL_ITEMS } else { ITEMS },
            uid_validity: 0,
            opening: None,
            opens: 0,
            pending: Vec::new(),
            email: config.email.clone(),
            listed,
            gmail,
        })
    }

    /// `command`, a `FETCH` of `self.items`, read by hand.
    async fn rows(
        &mut self,
        command: String,
        progress: impl Fn(u32),
    ) -> Result<Vec<NewMessage>, Error> {
        // What came before stays ahead of what comes during the fetch.
        let earlier = self.pushed();
        self.pending = earlier;
        let origin = Origin {
            uid_validity: self.uid_validity,
            email: &self.email,
        };

        new_messages::fetch(
            &mut self.session,
            command,
            origin,
            &mut self.pending,
            progress,
        )
        .await
    }

    /// `idle` for a server without `IDLE`: a `NOOP` after `POLL`, or once
    /// `stop` is done, collects what changed meanwhile.
    async fn poll(
        mut self,
        period: Duration,
        stop: impl Future<Output = ()>,
    ) -> Result<(Self, Vec<Pushed>), Error> {
        tokio::select! {
            () = tokio::time::sleep(period.min(POLL)) => {}
            () = stop => {}
        }

        self.session.noop().await?;
        let pushed = self.pushed();
        Ok((self, pushed))
    }

    fn opened(&mut self, mailbox: &async_imap::types::Mailbox, opening: Opening) -> Examined {
        self.uid_validity = mailbox.uid_validity.unwrap_or_default();
        self.opening = Some(opening.clone());
        Examined {
            opening,
            exists: mailbox.exists,
            state: SyncState {
                uid_validity: self.uid_validity,
                uid_next: mailbox.uid_next.map(Uid::new),
                highest_modseq: mailbox.highest_modseq,
            },
        }
    }

    async fn uid_fetch(&mut self, uids: String, items: &str) -> Result<Vec<Fetch>, Error> {
        let fetches = self.session.uid_fetch(uids, items).await?;
        Ok(fetches.try_collect().await?)
    }
}

impl MailServer for ImapSession {
    async fn open(&mut self, folder: &FolderName, mode: Mode) -> Result<(), Error> {
        if self
            .opening
            .as_ref()
            .is_some_and(|o| o.covers(folder, mode))
        {
            return Ok(());
        }

        self.reopen(folder, mode).await.map(drop)
    }

    async fn reopen(&mut self, folder: &FolderName, mode: Mode) -> Result<Examined, Error> {
        self.opening = None;
        self.pushed();
        self.opens += 1;
        let mailbox = match mode {
            // A plain EXAMINE suffices: a CONDSTORE server sends HIGHESTMODSEQ with
            // every EXAMINE (RFC 7162 3.1.2.1), and `(CONDSTORE)` is BAD elsewhere.
            Mode::Examine => self.session.examine(folder).await?,
            Mode::Select => self.session.select(folder).await?,
        };
        let opening = Opening::new(folder.clone(), mode, self.opens);
        Ok(self.opened(&mailbox, opening))
    }

    fn opening(&self) -> Option<&Opening> {
        self.opening.as_ref()
    }

    async fn store(&mut self, uids: &[Uid], flag: flag::Flag, added: bool) -> Result<(), Error> {
        // SILENT: the server need not echo the flags just set.
        let sign = if added { '+' } else { '-' };
        let query = format!("{sign}FLAGS.SILENT ({})", flag.name());
        let echoed: Vec<Fetch> = self
            .session
            .uid_store(uid_set(uids), &query)
            .await?
            .try_collect()
            .await?;
        tracing::info!("stored {query} on {} Messages", uids.len());
        drop(echoed);
        Ok(())
    }

    async fn move_to(&mut self, uids: &[Uid], to: &FolderName) -> Result<Vec<Moved>, Error> {
        let (set, to) = (uid_set(uids), quoted(to.as_str()));
        // What came before stays ahead of what comes during the move.
        let earlier = self.pushed();
        self.pending = earlier;
        let command = if self.listed.moves { "MOVE" } else { "COPY" };
        let command = format!("UID {command} {set} {to}");
        let moved = moved_uids::run(&mut self.session, command, &mut self.pending).await?;
        if !self.listed.moves {
            // The copy, flag and expunge a `MOVE` stands for (RFC 6851 3.3).
            let flagged: Vec<Fetch> = self
                .session
                .uid_store(&set, "+FLAGS.SILENT (\\Deleted)")
                .await?
                .try_collect()
                .await?;
            drop(flagged);
            if self.listed.uidplus {
                let pending = &mut self.pending;
                let expunge = format!("UID EXPUNGE {set}");
                tagged::run(&mut self.session, expunge, |response| {
                    pending.extend(Pushed::from_response(response));
                })
                .await?;
            } else {
                // A plain `EXPUNGE` would also take what another client flagged.
                tracing::warn!("no UIDPLUS: {} Messages left flagged \\Deleted", uids.len());
            }
        }

        tracing::info!("moved {} Messages", uids.len());
        Ok(moved)
    }

    async fn append(&mut self, folder: &FolderName, message: &[u8]) -> Result<(), Error> {
        self.session
            .append(folder, Some("(\\Seen)"), None, message)
            .await?;
        tracing::info!("saved a copy to {folder}");
        Ok(())
    }

    fn gmail(&self) -> bool {
        self.gmail
    }

    async fn folders(&mut self) -> Result<Vec<Folder<()>>, Error> {
        let names = self.session.list(Some(""), Some("*")).await?;
        let names: Vec<_> = names.try_collect().await?;
        let mut folders: Vec<_> = names
            .iter()
            .filter(|name| !name.attributes().contains(&NameAttribute::NoSelect))
            .map(|name| Folder::new((), name.name(), role(name.attributes())))
            .collect();
        // A `LIST` cut short by a dropped connection ends as if complete,
        // and every server lists INBOX (RFC 3501 5.1).
        if !folders
            .iter()
            .any(|f| f.name.as_str().eq_ignore_ascii_case(INBOX))
        {
            return Err(Error::Closed);
        }

        named_roles::fill(&mut folders);
        tracing::info!("listed {} Folders", folders.len());
        Ok(folders)
    }

    async fn unread_in(&mut self, folder: &FolderName) -> Result<usize, Error> {
        let status = self.session.status(folder, "(UNSEEN)").await?;
        Ok(status
            .unseen
            .map_or(0, |n| usize::try_from(n).unwrap_or(usize::MAX)))
    }

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

    async fn since_uid(&mut self, first: Uid) -> Result<Vec<NewMessage>, Error> {
        let command = format!("UID FETCH {first}:* {}", self.items);
        let mut messages = self.rows(command, |_| {}).await?;
        // `first:*` always matches the last Message, even below `first`.
        messages.retain(|m| m.row.uid >= first);
        tracing::info!("fetched {} new Messages", messages.len());
        Ok(messages)
    }

    async fn newest_envelopes(
        &mut self,
        exists: u32,
        count: u32,
    ) -> Result<Vec<(Uid, Correspondence)>, Error> {
        let first = exists.saturating_sub(count.saturating_sub(1)).max(1);
        let command = format!("FETCH {first}:* {ENVELOPE_ITEMS}");
        Ok(addressed(self.rows(command, |_| {}).await?, Uid::new(0)))
    }

    async fn envelopes_since(&mut self, first: Uid) -> Result<Vec<(Uid, Correspondence)>, Error> {
        let command = format!("UID FETCH {first}:* {ENVELOPE_ITEMS}");
        // `first:*` always matches the last Message, even below `first`.
        Ok(addressed(self.rows(command, |_| {}).await?, first))
    }

    async fn by_uid(&mut self, uids: &[Uid]) -> Result<Vec<NewMessage>, Error> {
        let command = format!("UID FETCH {} {}", uid_set(uids), self.items);
        let messages = self.rows(command, |_| {}).await?;
        tracing::info!("fetched {} Messages by UID", messages.len());
        Ok(messages)
    }

    async fn thread(&mut self, id: ThreadId) -> Result<Vec<Uid>, Error> {
        let found = self.session.uid_search(format!("X-GM-THRID {id}")).await?;
        let mut uids: Vec<_> = found.into_iter().map(Uid::new).collect();
        uids.sort_unstable();
        Ok(uids)
    }

    async fn sequence_number(&mut self, uid: Uid) -> Result<Option<u32>, Error> {
        let fetches = self.uid_fetch(uid.to_string(), "UID").await?;
        Ok(fetches
            .iter()
            .find(|f| f.uid == Some(uid.get()))
            .map(|f| f.message))
    }

    async fn uids(&mut self, first: Uid, last: Uid) -> Result<HashSet<Uid>, Error> {
        // `RETURN (ALL)` is out of reach, as for the Unread count:
        // see docs/adr/0004-unread-count-by-uid-search.md.
        Ok(self
            .session
            .uid_search(format!("UID {first}:{last}"))
            .await?
            .into_iter()
            .map(Uid::new)
            .collect())
    }

    async fn flags(
        &mut self,
        first: Uid,
        last: Uid,
        changed_since: Option<u64>,
    ) -> Result<Vec<(Uid, MessageFlags)>, Error> {
        let items = changed_since.map_or_else(
            || "(UID FLAGS)".to_owned(),
            |modseq| format!("(UID FLAGS) (CHANGEDSINCE {modseq})"),
        );
        let fetches = self.uid_fetch(format!("{first}:{last}"), &items).await?;
        let flags: Vec<_> = fetches
            .iter()
            .filter_map(|f| Some((Uid::new(f.uid?), flags(f))))
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
        uids: &[Uid],
        section: &str,
    ) -> Result<Vec<(Uid, Vec<u8>)>, Error> {
        let path = SectionPath::Part(
            section.split('.').filter_map(|n| n.parse().ok()).collect(),
            None,
        );
        // PEEK, or the server would mark every Message seen.
        let items = format!("(BODY.PEEK[{section}]<0.{}>)", snippet::FETCHED);
        let fetches = self.uid_fetch(uid_set(uids), &items).await?;
        Ok(fetches
            .iter()
            .filter_map(|f| Some((Uid::new(f.uid?), f.section(&path)?.to_vec())))
            .collect())
    }

    async fn bodies(&mut self, uids: &[Uid]) -> Result<Vec<(Uid, Vec<u8>)>, Error> {
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
        if !self.listed.idle {
            return self.poll(period, stop).await;
        }

        let Self {
            session,
            items,
            uid_validity,
            opening,
            opens,
            pending,
            email,
            listed,
            gmail,
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
            opening,
            opens,
            pending: Vec::new(),
            email,
            listed,
            gmail,
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

/// A Folder name as an IMAP quoted string.
fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
}

fn uid_set(uids: &[Uid]) -> String {
    uids.iter()
        .map(Uid::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn flags(fetch: &Fetch) -> MessageFlags {
    MessageFlags {
        unread: !fetch.flags().any(|flag| flag == Flag::Seen),
        answered: fetch.flags().any(|flag| flag == Flag::Answered),
    }
}

/// The UID and addresses of each of `rows` from `first` on.
fn addressed(rows: Vec<NewMessage>, first: Uid) -> Vec<(Uid, Correspondence)> {
    rows.into_iter()
        .filter(|m| m.row.uid >= first)
        .map(|m| (m.row.uid, m.correspondence))
        .collect()
}
