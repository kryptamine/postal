//! A scriptable IMAP server inside the test process, on `127.0.0.1:0`.
//! It serves an Inbox the test can change between runs and records every
//! command the client sends.

use std::{
    collections::HashMap,
    net::Ipv4Addr,
    num::{NonZeroU32, NonZeroU64},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use imap_next::{
    imap_types::{
        command::{Command, CommandBody},
        core::{Atom, QuotedChar, Tag, Vec1},
        fetch::MessageDataItem,
        flag::{Flag, FlagFetch, FlagNameAttribute},
        mailbox::Mailbox,
        response::{Code, CodeOther, CommandContinuationRequest, Data, Greeting, Status},
        search::SearchKey,
        status::{StatusDataItem, StatusDataItemName},
    },
    server::{Event, Options, Server},
    stream::Stream,
};
use tokio::net::{TcpListener, TcpStream};

use super::relay::relay;

mod appends;
mod auth;
mod fetch;
mod message;
mod moves;
mod stall;
mod store;

pub use auth::xoauth2;
use auth::{capabilities, finish_authenticate, login, start_authenticate};
use fetch::{answer_fetch, contains};
pub use message::FakeMessage;
pub use stall::Stall;
use stall::{wait_unless_command_stalled, wait_unless_stalled};
use store::store;

pub const EMAIL: &str = "me@example.com";
/// The OAuth access token `AUTHENTICATE XOAUTH2` accepts.
pub const ACCESS_TOKEN: &str = "ya29.fakeAccessToken";
/// The app-specific password `LOGIN` accepts, as iCloud formats one.
pub const APP_PASSWORD: &str = "abcd-efgh-ijkl-mnop";
/// The user `LOGIN` accepts unless told otherwise: the name part of `EMAIL`.
pub const LOGIN_USER: &str = "me";

/// The UID of the first Message given to `FakeImap::start`; the rest follow.
pub const FIRST_UID: u32 = 101;

/// The `X-GM-MSGID` of the first Message delivered; the rest follow. Past
/// `u32`, as Gmail's are.
pub const FIRST_MSGID: u64 = 1_800_000_000_000_000_001;

/// The `X-GM-THRID` of `FakeMessage::in_thread(0)`; the rest follow.
pub const FIRST_THRID: u64 = 1_700_000_000_000_000_001;

/// How the server turns clients away until `FakeImap::recover`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Outage {
    /// Each connection is closed before the greeting.
    Offline,
    /// Each connection is greeted with `BYE [UNAVAILABLE]`, as Gmail does
    /// while a backend is down.
    Bye,
    /// Every sign-in is answered `NO [THROTTLED]`.
    Throttled,
    /// Every sign-in is answered `NO [AUTHENTICATIONFAILED]`, as for a
    /// revoked access token.
    Revoked,
}

struct Stored {
    uid: u32,
    msgid: u64,
    modseq: u64,
    message: FakeMessage,
    /// Flagged `\Deleted`, for a `UID EXPUNGE` to take.
    deleted: bool,
}

impl Stored {
    fn thrid(&self) -> u64 {
        self.message.thread.map_or(self.msgid, |t| FIRST_THRID + t)
    }
}

pub const INBOX: &str = "INBOX";

/// One mailbox the server lists.
struct Folder {
    name: String,
    /// As `LIST` sends them, such as `\\Sent` or `\\Noselect`.
    attributes: Vec<&'static str>,
    uid_validity: u32,
    uid_next: u32,
    /// `None` serves the Folder without CONDSTORE.
    highest_modseq: Option<u64>,
    messages: Vec<Stored>,
    /// Gmail's All Mail: every delivery but to Trash and Spam lands here too.
    mirrors: bool,
}

impl Folder {
    fn new(name: &str, attributes: &[&'static str]) -> Self {
        Self {
            name: name.to_owned(),
            attributes: attributes.to_vec(),
            uid_validity: 1,
            uid_next: FIRST_UID,
            highest_modseq: Some(1),
            messages: Vec::new(),
            mirrors: false,
        }
    }

    fn bump(&mut self) -> u64 {
        let next = self.highest_modseq.unwrap_or_default() + 1;
        self.highest_modseq = self.highest_modseq.map(|_| next);
        next
    }

    /// Appends `message` under the next UID and returns that UID.
    fn push(&mut self, msgid: u64, message: FakeMessage) -> u32 {
        let uid = self.uid_next;
        let modseq = self.bump();
        self.uid_next += 1;
        self.messages.push(Stored {
            uid,
            msgid,
            modseq,
            message,
            deleted: false,
        });
        uid
    }

    fn trash(&self) -> bool {
        self.attributes.contains(&"\\Trash")
    }

    /// Kept out of All Mail: Trash, Spam and All Mail itself.
    fn apart(&self) -> bool {
        self.mirrors || self.attributes.contains(&"\\Trash") || self.attributes.contains(&"\\Junk")
    }

    fn stored(&mut self, uid: u32) -> &mut Stored {
        self.messages.iter_mut().find(|m| m.uid == uid).unwrap()
    }
}

/// What the server lists beside `UIDPLUS`, which is always listed.
struct Listed {
    moves: bool,
    /// Without it, `IDLE` is refused with `BAD`.
    idle: bool,
}

struct Inbox {
    /// The Inbox first.
    folders: Vec<Folder>,
    next_msgid: u64,
    /// Advertises `X-GM-EXT-1` and answers `X-GM-MSGID` and `X-GM-THRID`.
    gmail: bool,
    listed: Listed,
    /// The user name `LOGIN` takes with `APP_PASSWORD`.
    login_user: &'static str,
    stall: Option<Stall>,
    /// UIDs whose whole-body `UID FETCH` is answered `NO`, as Gmail does for
    /// a Message it cannot serve.
    refused: Vec<u32>,
    outage: Option<Outage>,
    /// Bumped to drop every connection open before.
    cut: u64,
    /// The `cut` each connection being served opened under.
    open: Vec<u64>,
    /// Each `APPEND`'s Folder and Message, in order.
    appended: Vec<(String, String)>,
    /// Drops the connection on each `APPEND` instead of answering it.
    cut_appends: bool,
}

impl Inbox {
    fn folder(&self, name: &str) -> Option<&Folder> {
        self.folders.iter().find(|f| f.name == name)
    }

    fn folder_mut(&mut self, name: &str) -> &mut Folder {
        self.folders.iter_mut().find(|f| f.name == name).unwrap()
    }

    fn inbox(&mut self) -> &mut Folder {
        self.folder_mut(INBOX)
    }
}

pub struct FakeImap {
    pub port: u16,
    commands: Arc<Mutex<Vec<String>>>,
    inbox: Arc<Mutex<Inbox>>,
}

impl FakeImap {
    /// The Inbox holds `messages` in sequence order: the first is sequence number 1.
    pub fn start(messages: Vec<FakeMessage>) -> Self {
        let std_listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = std_listener.local_addr().unwrap().port();
        std_listener.set_nonblocking(true).unwrap();
        let commands = Arc::new(Mutex::new(Vec::new()));
        let inbox = Arc::new(Mutex::new(Inbox {
            folders: vec![Folder::new(INBOX, &[])],
            next_msgid: FIRST_MSGID,
            gmail: true,
            listed: Listed {
                moves: true,
                idle: true,
            },
            login_user: LOGIN_USER,
            stall: None,
            outage: None,
            refused: Vec::new(),
            cut: 0,
            open: Vec::new(),
            appended: Vec::new(),
            cut_appends: false,
        }));
        let server = Self {
            port,
            commands: Arc::clone(&commands),
            inbox: Arc::clone(&inbox),
        };

        for message in messages {
            server.deliver(message);
        }

        thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(accept(std_listener, commands, inbox));
        });

        server
    }

    /// Each command as the client sent it, without its tag.
    pub fn commands(&self) -> Vec<String> {
        self.commands.lock().unwrap().clone()
    }

    pub fn clear_commands(&self) {
        self.commands.lock().unwrap().clear();
    }

    /// Appends a Message to the Inbox with the next UID and returns that UID.
    pub fn deliver(&self, message: FakeMessage) -> u32 {
        self.deliver_to(INBOX, message)
    }

    /// As `deliver`, to the Folder named `folder`, and a copy to All Mail if
    /// listed, unless `folder` is Trash or Spam.
    pub fn deliver_to(&self, folder: &str, message: FakeMessage) -> u32 {
        let mut inbox = self.inbox.lock().unwrap();
        let msgid = inbox.next_msgid;
        inbox.next_msgid += 1;
        if !inbox.folder_mut(folder).apart() {
            for all in inbox.folders.iter_mut().filter(|f| f.mirrors) {
                all.push(msgid, message.clone());
            }
        }

        let uid = inbox.folder_mut(folder).push(msgid, message);
        drop(inbox);
        uid
    }

    /// Lists a new Folder after the others, holding `messages` in sequence
    /// order; `attributes` as `LIST` sends them, such as `\\Sent`.
    pub fn add_folder(&self, name: &str, attributes: &[&'static str], messages: Vec<FakeMessage>) {
        self.inbox
            .lock()
            .unwrap()
            .folders
            .push(Folder::new(name, attributes));
        for message in messages {
            self.deliver_to(name, message);
        }
    }

    /// Lists Gmail's All Mail as `name`: a copy of every Message delivered,
    /// before or after, but those in Trash and Spam, under the same `X-GM-MSGID`.
    pub fn add_all_mail(&self, name: &str) {
        let mut inbox = self.inbox.lock().unwrap();
        let mut all = Folder::new(name, &["\\All"]);
        all.mirrors = true;
        let mut copies: Vec<_> = inbox
            .folders
            .iter()
            .filter(|f| !f.apart())
            .flat_map(|f| &f.messages)
            .map(|m| (m.msgid, m.message.clone()))
            .collect();
        copies.sort_by_key(|&(msgid, _)| msgid);
        for (msgid, message) in copies {
            all.push(msgid, message);
        }

        inbox.folders.push(all);
        drop(inbox);
    }

    /// Deletes the Folder, as another client may.
    pub fn remove_folder(&self, name: &str) {
        self.inbox
            .lock()
            .unwrap()
            .folders
            .retain(|f| f.name != name);
    }

    pub fn expunge(&self, uid: u32) {
        self.expunge_range(uid..=uid);
    }

    /// Expunges every UID in `uids` at once, so one IDLE announces them all.
    pub fn expunge_range(&self, uids: std::ops::RangeInclusive<u32>) {
        let mut state = self.inbox.lock().unwrap();
        let inbox = state.inbox();
        inbox.bump();
        inbox.messages.retain(|m| !uids.contains(&m.uid));
        drop(state);
    }

    pub fn set_seen(&self, uid: u32, seen: bool) {
        let mut state = self.inbox.lock().unwrap();
        let inbox = state.inbox();
        let modseq = inbox.bump();
        let stored = inbox.stored(uid);
        stored.modseq = modseq;
        stored.message.seen = seen;
        drop(state);
    }

    /// Whether the Message with this UID in `folder` has `\Seen`.
    pub fn seen(&self, folder: &str, uid: u32) -> bool {
        self.inbox
            .lock()
            .unwrap()
            .folder_mut(folder)
            .stored(uid)
            .message
            .seen
    }

    /// Whether the Message with this UID in `folder` has `\Answered`.
    pub fn answered(&self, folder: &str, uid: u32) -> bool {
        self.inbox
            .lock()
            .unwrap()
            .folder_mut(folder)
            .stored(uid)
            .message
            .answered
    }

    /// The `X-GM-MSGID` of the Message with this UID.
    pub fn msgid(&self, uid: u32) -> u64 {
        self.inbox.lock().unwrap().inbox().stored(uid).msgid
    }

    /// The subjects of the Messages in `folder`, in sequence order.
    pub fn subjects(&self, folder: &str) -> Vec<String> {
        let inbox = self.inbox.lock().unwrap();
        let subjects = inbox.folder(folder).unwrap().messages.iter();
        let subjects = subjects.map(|m| m.message.subject.clone()).collect();
        drop(inbox);
        subjects
    }

    /// Expunges every Message of `folder`, as emptying Trash elsewhere does.
    pub fn empty(&self, folder: &str) {
        let mut inbox = self.inbox.lock().unwrap();
        let folder = inbox.folder_mut(folder);
        folder.bump();
        folder.messages.clear();
        drop(inbox);
    }

    /// Each Message saved by `APPEND`, with the Folder it was saved to.
    pub fn appended(&self) -> Vec<(String, String)> {
        self.inbox.lock().unwrap().appended.clone()
    }

    /// Drops the connection on each `APPEND` from now on, or answers again.
    pub fn cut_appends(&self, cut: bool) {
        self.inbox.lock().unwrap().cut_appends = cut;
    }

    /// Serves later connections without `MOVE`, as a `UID COPY`, `\Deleted`
    /// and `UID EXPUNGE` stand in for it.
    pub fn without_move(&self) {
        self.inbox.lock().unwrap().listed.moves = false;
    }

    /// Serves later connections as a server without Gmail's extensions.
    pub fn without_gmail(&self) {
        self.inbox.lock().unwrap().gmail = false;
    }

    /// Serves later connections without `IDLE`.
    pub fn without_idle(&self) {
        self.inbox.lock().unwrap().listed.idle = false;
    }

    /// Takes `LOGIN` only as `user` from now on.
    pub fn login_as(&self, user: &'static str) {
        self.inbox.lock().unwrap().login_user = user;
    }

    /// Serves later connections as iCloud's IMAP does: no Gmail extensions
    /// and no `MOVE`, and Folders named for their use with no special-use
    /// attribute, as iCloud lists them without SPECIAL-USE.
    pub fn as_icloud(&self) {
        {
            let mut inbox = self.inbox.lock().unwrap();
            inbox.gmail = false;
            inbox.listed.moves = false;
        }

        for name in [
            "Sent Messages",
            "Drafts",
            "Junk",
            "Deleted Messages",
            "Archive",
        ] {
            self.add_folder(name, &[], Vec::new());
        }
    }

    /// Serves every Folder as a server without CONDSTORE: no `HIGHESTMODSEQ`.
    pub fn without_condstore(&self) {
        for folder in &mut self.inbox.lock().unwrap().folders {
            folder.highest_modseq = None;
        }
    }

    /// Replaces the Inbox under a new UIDVALIDITY, its UIDs starting at 1.
    pub fn renumber(&self, messages: Vec<FakeMessage>) {
        {
            let mut state = self.inbox.lock().unwrap();
            let inbox = state.inbox();
            inbox.uid_validity += 1;
            inbox.uid_next = 1;
            inbox.messages.clear();
            drop(state);
        }

        for message in messages {
            self.deliver(message);
        }
    }

    pub fn stall(&self, stall: Stall) {
        self.inbox.lock().unwrap().stall = Some(stall);
    }

    pub fn release(&self) {
        self.inbox.lock().unwrap().stall = None;
    }

    /// Drops every open connection and turns new ones away as `outage` says.
    /// Returns once they are all gone, so no later command reaches one.
    pub fn fail(&self, outage: Outage) {
        let cut = {
            let mut inbox = self.inbox.lock().unwrap();
            inbox.outage = Some(outage);
            inbox.cut += 1;
            inbox.cut
        };

        while self
            .inbox
            .lock()
            .unwrap()
            .open
            .iter()
            .any(|&open| open < cut)
        {
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn recover(&self) {
        self.inbox.lock().unwrap().outage = None;
    }

    pub fn refuse_body(&self, uid: u32) {
        self.inbox.lock().unwrap().refused.push(uid);
    }
}

/// Serves each client through a relay, until the listener fails.
async fn accept(
    std_listener: std::net::TcpListener,
    commands: Arc<Mutex<Vec<String>>>,
    inbox: Arc<Mutex<Inbox>>,
) {
    let listener = TcpListener::from_std(std_listener).unwrap();
    let inner = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let inner_address = inner.local_addr().unwrap();
    loop {
        let Ok((client, _)) = listener.accept().await else {
            return;
        };

        if inbox.lock().unwrap().outage == Some(Outage::Offline) {
            drop(client);
            continue;
        }

        let (to_fake, accepted) = tokio::join!(TcpStream::connect(inner_address), inner.accept());
        let (to_fake, (fake_side, _)) = (to_fake.unwrap(), accepted.unwrap());
        tokio::spawn(serve(Stream::insecure(fake_side), Arc::clone(&inbox)));
        let commands = Arc::clone(&commands);
        let inbox = Arc::clone(&inbox);
        let gmail = inbox.lock().unwrap().gmail;
        let record = move |command| commands.lock().unwrap().push(command);
        let msgid = move |folder: &str, uid| {
            let inbox = inbox.lock().unwrap();
            inbox
                .folder(folder)?
                .messages
                .iter()
                .find(|m| m.uid == uid)
                .map(|m| (m.msgid, m.thrid()))
        };

        tokio::spawn(relay(client, to_fake, record, gmail.then_some(msgid)));
    }
}

/// Counts a connection in `Inbox::open` while it is served.
struct Open {
    inbox: Arc<Mutex<Inbox>>,
    cut: u64,
}

impl Open {
    /// Counts a new connection open, with the outage it opened under.
    fn register(inbox: &Arc<Mutex<Inbox>>) -> (Self, Option<Outage>) {
        let (cut, outage) = {
            let mut inbox = inbox.lock().unwrap();
            let cut = inbox.cut;
            inbox.open.push(cut);
            (cut, inbox.outage)
        };

        let open = Self {
            inbox: Arc::clone(inbox),
            cut,
        };

        (open, outage)
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        let mut inbox = self.inbox.lock().unwrap();
        if let Some(at) = inbox.open.iter().position(|&open| open == self.cut) {
            inbox.open.remove(at);
        }
    }
}

async fn serve(mut stream: Stream, inbox: Arc<Mutex<Inbox>>) {
    wait_unless_stalled(&inbox, Stall::Greeting).await;
    let (open, outage) = Open::register(&inbox);
    let cut = open.cut;
    let bye = outage == Some(Outage::Bye);
    let mut server = Server::new(Options::default(), greeting(bye));
    let mut view = Vec::new();
    // Only a Folder `SELECT`ed takes a `STORE`.
    let mut opened = Opened {
        folder: INBOX.to_owned(),
        writable: false,
    };

    let mut idling = None;
    let mut authenticating = None;
    loop {
        let next = tokio::select! {
            next = stream.next(&mut server) => next,
            () = tokio::time::sleep(Duration::from_millis(10)) => {
                if !tick(&mut server, &mut view, &inbox, cut, idling.is_some().then_some(&*opened.folder)) {
                    return;
                }

                continue;
            }
        };

        let Ok(event) = next else {
            return;
        };

        match event {
            Event::GreetingSent { .. } if bye => return,
            Event::CommandAuthenticateReceived {
                command_authenticate,
            } => authenticating = start_authenticate(&mut server, command_authenticate),
            Event::AuthenticateDataReceived { authenticate_data } => {
                let outage = inbox.lock().unwrap().outage;
                let tag = authenticating.take();
                finish_authenticate(&mut server, tag, &authenticate_data, outage);
            }
            Event::IdleCommandReceived { tag } => {
                let listed = inbox.lock().unwrap().listed.idle;
                idling = start_idle(&mut server, tag, listed);
            }
            Event::IdleDoneReceived => {
                let done = Status::ok(idling.take(), None, "IDLE terminated").unwrap();
                server.enqueue_status(done);
            }
            Event::CommandReceived { command } => {
                if let Some(examined) = examined(&command.body, &inbox) {
                    (opened, view) = examined;
                }

                wait_unless_command_stalled(&inbox, &command.body).await;
                let appending = matches!(command.body, CommandBody::Append { .. });
                if appending && inbox.lock().unwrap().cut_appends {
                    return;
                }

                answer(&mut server, command, &inbox, &opened, &mut view);
            }
            _ => {}
        }
    }
}

/// Accepts `IDLE` where it is `listed`, returning the tag to end it under.
fn start_idle(server: &mut Server, tag: Tag<'static>, listed: bool) -> Option<Tag<'static>> {
    if !listed {
        let refused = Status::bad(Some(tag), None, "IDLE not listed").unwrap();
        server.idle_reject(refused).unwrap();
        return None;
    }

    let accept = CommandContinuationRequest::basic(None, "idling").unwrap();
    server.idle_accept(accept).unwrap();
    Some(tag)
}

/// Answers `command`, telling `view` what a move expunged.
fn answer(
    server: &mut Server,
    command: Command<'static>,
    inbox: &Mutex<Inbox>,
    opened: &Opened,
    view: &mut Vec<(u32, bool)>,
) {
    let mut inbox = inbox.lock().unwrap();
    if let CommandBody::Login { username, password } = &command.body {
        let status = login(&inbox, command.tag.clone(), username, password.declassify());
        server.enqueue_status(status);
    } else if matches!(command.body, CommandBody::Noop) {
        // A client without IDLE learns what changed from the NOOP's answer.
        if let Some(folder) = inbox.folder(&opened.folder) {
            announce(server, view, folder);
        }

        server.enqueue_status(Status::ok(Some(command.tag), None, "NOOP completed").unwrap());
    } else if matches!(command.body, CommandBody::Append { .. }) {
        let status = appends::append(command, &mut inbox);
        server.enqueue_status(status);
    } else if matches!(
        command.body,
        CommandBody::Move { .. } | CommandBody::Copy { .. } | CommandBody::ExpungeUid { .. }
    ) {
        let status = moves::transfer(server, command, &mut inbox, opened, view);
        server.enqueue_status(status);
    } else {
        if matches!(command.body, CommandBody::Store { .. })
            && let Some(folder) = inbox.folder(&opened.folder)
        {
            // A UID command may carry what other clients changed (RFC 3501 7.4.1).
            announce(server, view, folder);
        }

        respond(server, command, &mut inbox, opened);
    }

    drop(inbox);
}

/// The Folder opened last.
struct Opened {
    folder: String,
    /// `SELECT`ed rather than `EXAMINE`d.
    writable: bool,
}

/// The Folder an `EXAMINE` or a `SELECT` opens, with whether it was a
/// `SELECT`, and what an idling client is told of it.
fn examined(body: &CommandBody<'_>, inbox: &Mutex<Inbox>) -> Option<(Opened, Vec<(u32, bool)>)> {
    let (CommandBody::Examine { mailbox, .. } | CommandBody::Select { mailbox, .. }) = body else {
        return None;
    };

    let name = mailbox_name(mailbox);
    let view = inbox.lock().unwrap().folder(&name).map(seen);
    let writable = matches!(body, CommandBody::Select { .. });
    let opened = Opened {
        folder: name,
        writable,
    };

    Some((opened, view.unwrap_or_default()))
}

fn greeting(bye: bool) -> Greeting<'static> {
    let greeting = if bye {
        Greeting::bye(Some(code("UNAVAILABLE")), "Temporary System Error")
    } else {
        Greeting::ok(None, "fake IMAP ready")
    };

    greeting.unwrap()
}

/// Announces what changed to a client idling on `selected`; `false` once the
/// connection opened at `cut` is to be dropped.
fn tick(
    server: &mut Server,
    view: &mut Vec<(u32, bool)>,
    inbox: &Mutex<Inbox>,
    cut: u64,
    selected: Option<&str>,
) -> bool {
    let inbox = inbox.lock().unwrap();
    if inbox.cut != cut {
        return false;
    }

    if let Some(folder) = selected.and_then(|selected| inbox.folder(selected)) {
        announce(server, view, folder);
    }

    drop(inbox);
    true
}

/// Each Message's UID and `\Seen`, in sequence order.
fn seen(folder: &Folder) -> Vec<(u32, bool)> {
    folder
        .messages
        .iter()
        .map(|m| (m.uid, m.message.seen))
        .collect()
}

/// Tells an idling client what changed since its `view`, as a server does:
/// expunges lowest first, each renumbering the rest, then flags without the
/// UID, then the new count. Only a change of `\Seen` is announced.
fn announce(server: &mut Server, view: &mut Vec<(u32, bool)>, folder: &Folder) {
    let now = seen(folder);
    let current: HashMap<_, _> = now.iter().copied().collect();
    let mut index = 0;
    while index < view.len() {
        if current.contains_key(&view[index].0) {
            index += 1;
            continue;
        }

        view.remove(index);
        let number = NonZeroU32::new(u32::try_from(index + 1).unwrap()).unwrap();
        server.enqueue_data(Data::Expunge(number));
    }

    for (number, (uid, seen)) in (1..).zip(view.iter_mut()) {
        let current = current[uid];
        if current != *seen {
            *seen = current;
            let answered = folder
                .messages
                .iter()
                .any(|m| m.uid == *uid && m.message.answered);
            server.enqueue_data(Data::Fetch {
                seq: NonZeroU32::new(number).unwrap(),
                items: Vec1::from(MessageDataItem::Flags(flags(current, answered))),
            });
        }
    }

    let last = view.last().map_or(0, |&(uid, _)| uid);
    let before = view.len();
    view.extend(now.into_iter().filter(|&(uid, _)| uid > last));
    if view.len() > before {
        server.enqueue_data(Data::Exists(u32::try_from(view.len()).unwrap()));
    }
}

fn flags(seen: bool, answered: bool) -> Vec<FlagFetch<'static>> {
    [(seen, Flag::Seen), (answered, Flag::Answered)]
        .into_iter()
        .filter(|&(set, _)| set)
        .map(|(_, flag)| FlagFetch::Flag(flag))
        .collect()
}

/// A response code imap-types has no variant for, such as Gmail's.
fn code(atom: &'static str) -> Code<'static> {
    Code::Other(CodeOther::unvalidated(atom.as_bytes()))
}

/// The name a command gives a mailbox, as `LIST` sends it.
fn mailbox_name(mailbox: &Mailbox<'_>) -> String {
    match mailbox {
        Mailbox::Inbox => INBOX.to_owned(),
        Mailbox::Other(other) => String::from_utf8(other.as_ref().to_vec()).unwrap(),
    }
}

/// The Folder `body` reads, `None` when it is gone. The Inbox stands in
/// for a gone one where no command reads it.
fn selected_folder<'a>(
    body: &CommandBody<'_>,
    inbox: &'a Inbox,
    selected: &str,
) -> Option<&'a Folder> {
    let reads = matches!(
        body,
        CommandBody::Examine { .. }
            | CommandBody::Select { .. }
            | CommandBody::Search { .. }
            | CommandBody::Fetch { .. }
    );
    match inbox.folder(selected) {
        None if !reads => inbox.folders.first(),
        found => found,
    }
}

fn respond(server: &mut Server, command: Command<'static>, inbox: &mut Inbox, opened: &Opened) {
    if let CommandBody::Store { .. } = command.body {
        server.enqueue_status(store(inbox, command, opened));
        return;
    }

    let inbox = &*inbox;
    let tag = Some(command.tag);
    let Some(folder) = selected_folder(&command.body, inbox, &opened.folder) else {
        let status = Status::no(tag, Some(code("NONEXISTENT")), "Unknown Mailbox");
        server.enqueue_status(status.unwrap());
        return;
    };

    match command.body {
        CommandBody::Capability => {
            server.enqueue_data(capabilities(inbox));
            server.enqueue_status(Status::ok(tag, None, "CAPABILITY completed").unwrap());
        }
        body @ (CommandBody::List { .. } | CommandBody::Status { .. }) => {
            let status = answer_folders(server, tag, body, inbox);
            server.enqueue_status(status);
        }
        CommandBody::Examine { .. } | CommandBody::Select { .. } => {
            examine(server, folder, tag, opened.writable);
        }
        CommandBody::Search {
            criteria,
            uid: true,
            ..
        } => match search(criteria.as_ref(), folder) {
            Some(uids) => {
                let uids = uids.into_iter().map(|uid| NonZeroU32::new(uid).unwrap());
                server.enqueue_data(Data::Search(uids.collect(), None));
                server.enqueue_status(Status::ok(tag, None, "SEARCH completed").unwrap());
            }
            None => {
                server.enqueue_status(Status::bad(tag, None, "not faked").unwrap());
            }
        },
        body @ CommandBody::Fetch { .. } => {
            let status = answer_fetch(server, tag, body, inbox, folder);
            server.enqueue_status(status);
        }
        _ => {
            server.enqueue_status(Status::bad(tag, None, "not faked").unwrap());
        }
    }
}

/// Enqueues what a `LIST` or a `STATUS` asks for and returns its tagged status.
fn answer_folders(
    server: &mut Server,
    tag: Option<Tag<'static>>,
    body: CommandBody<'static>,
    inbox: &Inbox,
) -> Status<'static> {
    let CommandBody::Status {
        mailbox,
        item_names,
    } = body
    else {
        for folder in &inbox.folders {
            server.enqueue_data(listed(folder));
        }

        return Status::ok(tag, None, "LIST completed").unwrap();
    };

    let Some(folder) = inbox.folder(&mailbox_name(&mailbox)) else {
        return Status::no(tag, Some(code("NONEXISTENT")), "Unknown Mailbox").unwrap();
    };

    let unseen = folder.messages.iter().filter(|m| !m.message.seen).count();
    let items = item_names.iter().filter_map(|name| match name {
        StatusDataItemName::Unseen => Some(StatusDataItem::Unseen(u32::try_from(unseen).unwrap())),
        StatusDataItemName::UidNext => Some(StatusDataItem::UidNext(
            NonZeroU32::new(folder.uid_next).unwrap(),
        )),
        _ => None,
    });

    server.enqueue_data(Data::Status {
        mailbox,
        items: items.collect::<Vec<_>>().into(),
    });
    Status::ok(tag, None, "STATUS completed").unwrap()
}

/// The `LIST` line of `folder`, `/` its hierarchy delimiter as on Gmail.
fn listed(folder: &Folder) -> Data<'static> {
    let items = folder
        .attributes
        .iter()
        .map(|a| FlagNameAttribute::from(Atom::try_from(a.trim_start_matches('\\')).unwrap()))
        .collect();
    Data::List {
        items,
        delimiter: Some(QuotedChar::try_from('/').unwrap()),
        mailbox: Mailbox::try_from(folder.name.clone()).unwrap(),
    }
}

/// Answers `tag` with `READ-WRITE` when `writable`, as for a `SELECT`.
fn examine(server: &mut Server, inbox: &Folder, tag: Option<Tag<'static>>, writable: bool) {
    let exists = u32::try_from(inbox.messages.len()).unwrap();
    server.enqueue_data(Data::Exists(exists));
    server.enqueue_data(Data::Flags(vec![Flag::Seen]));
    let mut codes = vec![
        Code::UidValidity(NonZeroU32::new(inbox.uid_validity).unwrap()),
        Code::UidNext(NonZeroU32::new(inbox.uid_next).unwrap()),
    ];
    if let Some(modseq) = inbox.highest_modseq {
        codes.push(Code::HighestModSeq(NonZeroU64::new(modseq).unwrap()));
    }

    for code in codes {
        server.enqueue_status(Status::ok(None, Some(code), "fake").unwrap());
    }

    let code = if writable {
        Code::ReadWrite
    } else {
        Code::ReadOnly
    };
    server.enqueue_status(Status::ok(tag, Some(code), "opened").unwrap());
}

/// The UIDs a `UID SEARCH` matches; `None` for criteria the fake does not know.
fn search(criteria: &[SearchKey<'_>], inbox: &Folder) -> Option<Vec<u32>> {
    let max = inbox.messages.last().map_or(0, |m| m.uid);
    let mut uids = Vec::new();
    for stored in &inbox.messages {
        let matches = match criteria {
            [SearchKey::Unseen] => !stored.message.seen,
            [SearchKey::Uid(set)] => contains(set, stored.uid, max),
            // The relay's stand-in for `X-GM-THRID n`.
            [SearchKey::Header(field, value)] if field.as_ref() == b"X-GM-THRID" => {
                value.as_ref() == stored.thrid().to_string().as_bytes()
            }
            _ => return None,
        };

        if matches {
            uids.push(stored.uid);
        }
    }

    Some(uids)
}
