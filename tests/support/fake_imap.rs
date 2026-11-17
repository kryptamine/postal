//! A scriptable IMAP server inside the test process, on `127.0.0.1:0`.
//! It serves an Inbox the test can change between runs and records every
//! command the client sends.

use std::{
    net::Ipv4Addr,
    num::{NonZeroU32, NonZeroU64},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use imap_codec::{CommandCodec, encode::Encoder};
use imap_next::{
    imap_types::{
        command::{Command, CommandBody, FetchModifier},
        core::{IString, NString, Vec1},
        datetime::DateTime,
        envelope::{Address, Envelope},
        fetch::{MacroOrMessageDataItemNames, MessageDataItem, MessageDataItemName, Part, Section},
        flag::{Flag, FlagFetch},
        response::{Code, Data, Greeting, Status},
        search::SearchKey,
        sequence::{SeqOrUid, Sequence, SequenceSet},
    },
    server::{Event, Options, Server},
    stream::Stream,
};
use tokio::net::TcpListener;

use super::fake_body::FakeBody;

pub const EMAIL: &str = "me@example.com";
pub const PASSWORD: &str = "abcd efgh ijkl mnop";

/// The UID of the first Message given to `FakeImap::start`; the rest follow.
pub const FIRST_UID: u32 = 101;

#[derive(Clone)]
pub struct FakeMessage {
    pub from_name: Option<&'static str>,
    pub from_address: &'static str,
    /// A raw `From` header the server could not parse: the envelope then has no `From`.
    pub unparsable_from: Option<&'static str>,
    /// Raw header text, so a test can send RFC 2047 encoded words.
    pub subject: String,
    /// RFC 3339.
    pub internal_date: String,
    pub seen: bool,
    pub body: FakeBody,
}

impl FakeMessage {
    pub fn new(from_name: &'static str, subject: &str, internal_date: &str) -> Self {
        Self {
            from_name: Some(from_name),
            from_address: "someone@example.com",
            unparsable_from: None,
            subject: subject.to_owned(),
            internal_date: internal_date.to_owned(),
            seen: true,
            body: FakeBody::plain(""),
        }
    }

    pub fn body(mut self, body: FakeBody) -> Self {
        self.body = body;
        self
    }

    pub const fn unread(mut self) -> Self {
        self.seen = false;
        self
    }

    pub const fn unparsable_from(mut self, raw: &'static str) -> Self {
        self.unparsable_from = Some(raw);
        self
    }

    /// The `From` header's value as the Message carries it.
    fn sender_header(&self) -> String {
        match (self.unparsable_from, self.from_name) {
            (Some(raw), _) => raw.to_owned(),
            (None, Some(name)) => format!("{name} <{}>", self.from_address),
            (None, None) => self.from_address.to_owned(),
        }
    }
}

/// Where the server stops answering until `FakeImap::release`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stall {
    /// Before the greeting: the client sees no byte at all.
    Greeting,
    /// After a `FETCH` or `UID FETCH` is recorded, before its answer.
    Fetch,
    /// As `Fetch`, but only for a fetch of a body part.
    Part,
}

struct Stored {
    uid: u32,
    modseq: u64,
    message: FakeMessage,
}

struct Inbox {
    uid_validity: u32,
    uid_next: u32,
    /// `None` serves the Inbox without CONDSTORE.
    highest_modseq: Option<u64>,
    messages: Vec<Stored>,
    stall: Option<Stall>,
}

impl Inbox {
    fn bump(&mut self) -> u64 {
        let next = self.highest_modseq.unwrap_or_default() + 1;
        self.highest_modseq = self.highest_modseq.map(|_| next);
        next
    }

    fn stored(&mut self, uid: u32) -> &mut Stored {
        self.messages.iter_mut().find(|m| m.uid == uid).unwrap()
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
            uid_validity: 1,
            uid_next: FIRST_UID,
            highest_modseq: Some(1),
            messages: Vec::new(),
            stall: None,
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
            runtime.block_on(async move {
                let listener = TcpListener::from_std(std_listener).unwrap();
                loop {
                    let Ok((tcp, _)) = listener.accept().await else {
                        return;
                    };

                    let commands = Arc::clone(&commands);
                    let inbox = Arc::clone(&inbox);
                    tokio::spawn(serve(Stream::insecure(tcp), inbox, commands));
                }
            });
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

    /// Appends a Message with the next UID and returns that UID.
    pub fn deliver(&self, message: FakeMessage) -> u32 {
        let mut inbox = self.inbox.lock().unwrap();
        let uid = inbox.uid_next;
        let modseq = inbox.bump();
        inbox.uid_next += 1;
        inbox.messages.push(Stored {
            uid,
            modseq,
            message,
        });
        uid
    }

    pub fn expunge(&self, uid: u32) {
        let mut inbox = self.inbox.lock().unwrap();
        inbox.bump();
        inbox.messages.retain(|m| m.uid != uid);
    }

    pub fn set_seen(&self, uid: u32, seen: bool) {
        let mut inbox = self.inbox.lock().unwrap();
        let modseq = inbox.bump();
        let stored = inbox.stored(uid);
        stored.modseq = modseq;
        stored.message.seen = seen;
        drop(inbox);
    }

    /// Serves the Inbox as a server without CONDSTORE: no `HIGHESTMODSEQ`.
    pub fn without_condstore(&self) {
        self.inbox.lock().unwrap().highest_modseq = None;
    }

    /// Replaces the Inbox under a new UIDVALIDITY, its UIDs starting at 1.
    pub fn renumber(&self, messages: Vec<FakeMessage>) {
        {
            let mut inbox = self.inbox.lock().unwrap();
            inbox.uid_validity += 1;
            inbox.uid_next = 1;
            inbox.messages.clear();
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
}

async fn wait_unless_stalled(inbox: &Mutex<Inbox>, stall: Stall) {
    while inbox.lock().unwrap().stall == Some(stall) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn serve(mut stream: Stream, inbox: Arc<Mutex<Inbox>>, recorded: Arc<Mutex<Vec<String>>>) {
    wait_unless_stalled(&inbox, Stall::Greeting).await;
    let greeting = Greeting::ok(None, "fake IMAP ready").unwrap();
    let mut server = Server::new(Options::default(), greeting);
    let mut logging_out = false;
    loop {
        let Ok(event) = stream.next(&mut server).await else {
            return;
        };

        match event {
            Event::CommandReceived { command } => {
                recorded.lock().unwrap().push(untagged(&command));
                logging_out = matches!(command.body, CommandBody::Logout);
                if let CommandBody::Fetch {
                    macro_or_item_names,
                    ..
                } = &command.body
                {
                    wait_unless_stalled(&inbox, Stall::Fetch).await;
                    if body_part(macro_or_item_names).is_some() {
                        wait_unless_stalled(&inbox, Stall::Part).await;
                    }
                }

                respond(&mut server, command, &inbox.lock().unwrap());
            }
            Event::ResponseSent { response, .. } if logging_out => {
                if matches!(
                    response,
                    imap_next::imap_types::response::Response::Status(Status::Tagged(_))
                ) {
                    return;
                }
            }
            _ => {}
        }
    }
}

fn untagged(command: &Command<'_>) -> String {
    let line = String::from_utf8(CommandCodec::default().encode(command).dump()).unwrap();
    let line = line.trim_end_matches("\r\n");
    line.split_once(' ')
        .map_or(line, |(_, rest)| rest)
        .to_owned()
}

fn respond(server: &mut Server, command: Command<'static>, inbox: &Inbox) {
    let tag = Some(command.tag);
    match command.body {
        CommandBody::Login { username, password } => {
            let ok = username.as_ref() == EMAIL.as_bytes()
                && password.declassify().as_ref() == PASSWORD.as_bytes();
            let status = if ok {
                Status::ok(tag, None, "LOGIN completed")
            } else {
                Status::no(tag, None, "Invalid credentials")
            };

            server.enqueue_status(status.unwrap());
        }
        CommandBody::Examine { .. } | CommandBody::Select { .. } => {
            examine(server, inbox);
            server.enqueue_status(
                Status::ok(tag, Some(Code::ReadOnly), "EXAMINE completed").unwrap(),
            );
        }
        CommandBody::Search {
            criteria,
            uid: true,
            ..
        } => match search(criteria.as_ref(), inbox) {
            Some(uids) => {
                let uids = uids.into_iter().map(|uid| NonZeroU32::new(uid).unwrap());
                server.enqueue_data(Data::Search(uids.collect(), None));
                server.enqueue_status(Status::ok(tag, None, "SEARCH completed").unwrap());
            }
            None => {
                server.enqueue_status(Status::bad(tag, None, "not faked").unwrap());
            }
        },
        CommandBody::Fetch {
            sequence_set,
            macro_or_item_names,
            uid,
            modifiers,
        } => {
            let asked = Asked {
                from_header: header_fields(&macro_or_item_names),
                part: body_part(&macro_or_item_names),
            };
            let changed_since = modifiers.iter().find_map(|m| match m {
                FetchModifier::ChangedSince(modseq) => Some(modseq.get()),
                FetchModifier::Vanished => None,
            });
            for (seq, stored) in fetched(inbox, &sequence_set, uid, changed_since) {
                server.enqueue_data(fetch(seq, stored, &asked));
            }

            server.enqueue_status(Status::ok(tag, None, "FETCH completed").unwrap());
        }
        CommandBody::Logout => {
            server.enqueue_status(Status::bye(None, "logging out").unwrap());
            server.enqueue_status(Status::ok(tag, None, "LOGOUT completed").unwrap());
        }
        _ => {
            server.enqueue_status(Status::bad(tag, None, "not faked").unwrap());
        }
    }
}

fn examine(server: &mut Server, inbox: &Inbox) {
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
}

/// The UIDs a `UID SEARCH` matches; `None` for criteria the fake does not know.
fn search(criteria: &[SearchKey<'_>], inbox: &Inbox) -> Option<Vec<u32>> {
    let max = inbox.messages.last().map_or(0, |m| m.uid);
    let mut uids = Vec::new();
    for stored in &inbox.messages {
        let matches = match criteria {
            [SearchKey::Unseen] => !stored.message.seen,
            [SearchKey::Uid(set)] => contains(set, stored.uid, max),
            _ => return None,
        };

        if matches {
            uids.push(stored.uid);
        }
    }

    Some(uids)
}

/// The Messages a `FETCH` names, with their sequence numbers.
fn fetched<'a>(
    inbox: &'a Inbox,
    set: &'a SequenceSet,
    uid: bool,
    changed_since: Option<u64>,
) -> impl Iterator<Item = (u32, &'a Stored)> {
    let max = if uid {
        inbox.messages.last().map_or(0, |m| m.uid)
    } else {
        u32::try_from(inbox.messages.len()).unwrap()
    };

    (1..).zip(&inbox.messages).filter(move |&(seq, stored)| {
        contains(set, if uid { stored.uid } else { seq }, max)
            && changed_since.is_none_or(|since| stored.modseq > since)
    })
}

/// Whether `set` holds `value`, with `*` standing for `max`.
fn contains(set: &SequenceSet, value: u32, max: u32) -> bool {
    let resolve = |bound: &SeqOrUid| match bound {
        SeqOrUid::Value(n) => n.get(),
        SeqOrUid::Asterisk => max,
    };

    set.0.as_ref().iter().any(|sequence| match sequence {
        Sequence::Single(one) => resolve(one) == value,
        Sequence::Range(a, b) => {
            let (a, b) = (resolve(a), resolve(b));
            (a.min(b)..=a.max(b)).contains(&value)
        }
    })
}

/// The `HEADER.FIELDS` section the client asked for, if any; the fake serves
/// only the `From` field whatever the list names.
fn header_fields(items: &MacroOrMessageDataItemNames<'static>) -> Option<Section<'static>> {
    let MacroOrMessageDataItemNames::MessageDataItemNames(names) = items else {
        return None;
    };

    names.iter().find_map(|name| match name {
        MessageDataItemName::BodyExt {
            section: Some(section @ Section::HeaderFields(..)),
            ..
        } => Some(section.clone()),
        _ => None,
    })
}

/// A body part number the client asked for, and the octets it asked for.
struct PartAsked {
    path: Vec1<NonZeroU32>,
    partial: Option<(u32, NonZeroU32)>,
}

/// The items of a `FETCH` the fake answers only when asked.
struct Asked {
    from_header: Option<Section<'static>>,
    part: Option<PartAsked>,
}

fn body_part(items: &MacroOrMessageDataItemNames<'static>) -> Option<PartAsked> {
    let MacroOrMessageDataItemNames::MessageDataItemNames(names) = items else {
        return None;
    };

    names.iter().find_map(|name| match name {
        MessageDataItemName::BodyExt {
            section: Some(Section::Part(Part(path))),
            partial,
            ..
        } => Some(PartAsked {
            path: path.clone(),
            partial: *partial,
        }),
        _ => None,
    })
}

/// The octets asked for, empty for a part the body does not have.
fn body_part_item(body: &FakeBody, asked: &PartAsked) -> MessageDataItem<'static> {
    let content = body.part(asked.path.as_ref()).unwrap_or_default();
    let (start, len) = asked.partial.map_or((0, content.len()), |(start, len)| {
        (start as usize, len.get() as usize)
    });
    let served = content.get(start..).unwrap_or_default();
    let served = &served[..len.min(served.len())];
    MessageDataItem::BodyExt {
        section: Some(Section::Part(Part(asked.path.clone()))),
        origin: asked.partial.map(|(start, _)| start),
        // A short part goes quoted, as a server may send it.
        data: NString(Some(IString::try_from(served.to_vec()).unwrap())),
    }
}

/// Every item whatever the client asked for, as a client must cope with extras.
fn fetch(seq: u32, stored: &Stored, asked: &Asked) -> Data<'static> {
    let message = &stored.message;
    let nstring = |s: &str| NString(Some(IString::try_from(s.to_owned()).unwrap()));
    let (mailbox, host) = message.from_address.split_once('@').unwrap();
    let from = Address {
        name: message.from_name.map_or(NString(None), nstring),
        adl: NString(None),
        mailbox: nstring(mailbox),
        host: nstring(host),
    };

    let from = if message.unparsable_from.is_some() {
        vec![]
    } else {
        vec![from]
    };

    let envelope = Envelope {
        date: NString(None),
        subject: nstring(&message.subject),
        from: from.clone(),
        sender: from.clone(),
        reply_to: from,
        to: vec![],
        cc: vec![],
        bcc: vec![],
        in_reply_to: NString(None),
        message_id: NString(None),
    };

    let flags = if message.seen {
        vec![FlagFetch::Flag(Flag::Seen)]
    } else {
        vec![]
    };
    let received = chrono::DateTime::parse_from_rfc3339(&message.internal_date).unwrap();
    let mut items = vec![
        MessageDataItem::Uid(NonZeroU32::new(stored.uid).unwrap()),
        MessageDataItem::Flags(flags),
        MessageDataItem::InternalDate(DateTime::try_from(received).unwrap()),
        MessageDataItem::Envelope(envelope),
        MessageDataItem::ModSeq(NonZeroU64::new(stored.modseq).unwrap()),
        MessageDataItem::BodyStructure(message.body.structure()),
    ];
    if let Some(asked) = &asked.part {
        items.push(body_part_item(&message.body, asked));
    }

    if let Some(section) = &asked.from_header {
        items.push(MessageDataItem::BodyExt {
            section: Some(section.clone()),
            origin: None,
            data: nstring(&format!("From: {}\r\n\r\n", message.sender_header())),
        });
    }

    Data::Fetch {
        seq: NonZeroU32::new(seq).unwrap(),
        items: Vec1::try_from(items).unwrap(),
    }
}
