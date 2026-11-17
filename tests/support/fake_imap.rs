//! A scriptable IMAP server inside the test process, on `127.0.0.1:0`.
//! It serves a fixed Inbox and records every command the client sends.

use std::{
    net::Ipv4Addr,
    num::NonZeroU32,
    sync::{Arc, Mutex},
    thread,
};

use imap_codec::{CommandCodec, encode::Encoder};
use imap_next::{
    imap_types::{
        command::{Command, CommandBody},
        core::{IString, NString, Vec1},
        datetime::DateTime,
        envelope::{Address, Envelope},
        fetch::{MacroOrMessageDataItemNames, MessageDataItem, MessageDataItemName, Section},
        flag::{Flag, FlagFetch},
        response::{Code, Data, Greeting, Status},
        search::SearchKey,
        sequence::{SeqOrUid, Sequence},
    },
    server::{Event, Options, Server},
    stream::Stream,
};
use tokio::net::TcpListener;

pub const EMAIL: &str = "me@example.com";
pub const PASSWORD: &str = "abcd efgh ijkl mnop";

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
        }
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

pub struct FakeImap {
    pub port: u16,
    commands: Arc<Mutex<Vec<String>>>,
}

impl FakeImap {
    /// The Inbox holds `messages` in sequence order: the first is sequence number 1.
    pub fn start(messages: Vec<FakeMessage>) -> Self {
        let std_listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = std_listener.local_addr().unwrap().port();
        std_listener.set_nonblocking(true).unwrap();
        let commands = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&commands);
        let messages = Arc::new(messages);
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

                    let recorded = Arc::clone(&recorded);
                    let messages = Arc::clone(&messages);
                    tokio::spawn(serve(Stream::insecure(tcp), messages, recorded));
                }
            });
        });

        Self { port, commands }
    }

    /// Each command as the client sent it, without its tag.
    pub fn commands(&self) -> Vec<String> {
        self.commands.lock().unwrap().clone()
    }
}

async fn serve(
    mut stream: Stream,
    messages: Arc<Vec<FakeMessage>>,
    recorded: Arc<Mutex<Vec<String>>>,
) {
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
                respond(&mut server, command, &messages);
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

fn respond(server: &mut Server, command: Command<'static>, messages: &[FakeMessage]) {
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
            let exists = u32::try_from(messages.len()).unwrap();
            server.enqueue_data(Data::Exists(exists));
            server.enqueue_data(Data::Flags(vec![Flag::Seen]));
            server.enqueue_status(
                Status::ok(tag, Some(Code::ReadOnly), "EXAMINE completed").unwrap(),
            );
        }
        CommandBody::Search {
            criteria,
            uid: true,
            ..
        } if matches!(criteria.as_ref(), [SearchKey::Unseen]) => {
            server.enqueue_data(Data::Search(unseen(messages)));
            server.enqueue_status(Status::ok(tag, None, "SEARCH completed").unwrap());
        }
        CommandBody::Fetch {
            sequence_set,
            macro_or_item_names,
            ..
        } => {
            let from_header = header_fields(&macro_or_item_names);
            let max = u32::try_from(messages.len()).unwrap();
            for sequence in sequence_set.0.as_ref() {
                let (first, last) = match sequence {
                    Sequence::Single(one) => (resolve(*one, max), resolve(*one, max)),
                    Sequence::Range(a, b) => {
                        let (a, b) = (resolve(*a, max), resolve(*b, max));
                        (a.min(b), a.max(b))
                    }
                };

                for seq in first..=last {
                    let message = &messages[usize::try_from(seq - 1).unwrap()];
                    server.enqueue_data(fetch(seq, message, from_header.as_ref()));
                }
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

const fn resolve(value: SeqOrUid, max: u32) -> u32 {
    match value {
        SeqOrUid::Value(n) => n.get(),
        SeqOrUid::Asterisk => max,
    }
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

const fn uid(seq: u32) -> NonZeroU32 {
    NonZeroU32::new(seq + 100).unwrap()
}

fn unseen(messages: &[FakeMessage]) -> Vec<NonZeroU32> {
    (1..)
        .zip(messages)
        .filter(|(_, m)| !m.seen)
        .map(|(seq, _)| uid(seq))
        .collect()
}

fn fetch(seq: u32, message: &FakeMessage, from_header: Option<&Section<'static>>) -> Data<'static> {
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
        MessageDataItem::Uid(uid(seq)),
        MessageDataItem::Flags(flags),
        MessageDataItem::InternalDate(DateTime::try_from(received).unwrap()),
        MessageDataItem::Envelope(envelope),
    ];
    if let Some(section) = from_header {
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
