//! What a `FETCH` asks for, and the items the fake answers it with.

use std::{
    fmt::Write,
    num::{NonZeroU32, NonZeroU64},
};

use imap_next::{
    imap_types::{
        command::{CommandBody, FetchModifier},
        core::{IString, Literal, NString, Tag, Vec1},
        datetime::DateTime,
        envelope::{Address, Envelope},
        fetch::{MacroOrMessageDataItemNames, MessageDataItem, MessageDataItemName, Part, Section},
        response::{Data, Status},
        sequence::{SeqOrUid, Sequence, SequenceSet},
    },
    server::Server,
};

use super::{Folder, Inbox, Stored, flags, message::FakeMessage};

/// Enqueues what a `FETCH` asks for and returns its tagged status.
pub(super) fn answer_fetch(
    server: &mut Server,
    tag: Option<Tag<'static>>,
    body: CommandBody<'static>,
    inbox: &Inbox,
    folder: &Folder,
) -> Status<'static> {
    let CommandBody::Fetch {
        sequence_set,
        macro_or_item_names,
        uid,
        modifiers,
    } = body
    else {
        panic!("only a FETCH is answered here");
    };

    let asked = Asked {
        header_fields: header_fields(&macro_or_item_names),
        parts: body_parts(&macro_or_item_names),
        whole: whole_body(&macro_or_item_names),
    };

    // As iCloud: numbers count the Folder as it is now, whatever this
    // connection was told.
    if !uid && beyond(&sequence_set, u32::try_from(folder.messages.len()).unwrap()) {
        return Status::no(tag, None, "Invalid message number").unwrap();
    }

    let refused = fetched(folder, &sequence_set, uid, None)
        .any(|(_, stored)| inbox.refused.contains(&stored.uid));
    // As Gmail: a Message it cannot serve refuses any section of its body.
    if (asked.whole || !asked.parts.is_empty()) && refused {
        let text = "Some messages could not be FETCHed (Failure)";
        return Status::no(tag, None, text).unwrap();
    }

    let changed_since = modifiers.iter().find_map(|m| match m {
        FetchModifier::ChangedSince(modseq) => Some(modseq.get()),
        FetchModifier::Vanished => None,
    });
    for (seq, stored) in fetched(folder, &sequence_set, uid, changed_since) {
        server.enqueue_data(fetch(seq, stored, &asked));
    }

    Status::ok(tag, None, "FETCH completed").unwrap()
}

/// The Messages a `FETCH` names, with their sequence numbers.
pub(super) fn fetched<'a>(
    inbox: &'a Folder,
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

/// Whether `set` names a number past `count`; `*` never does.
fn beyond(set: &SequenceSet, count: u32) -> bool {
    let past = |bound: &SeqOrUid| matches!(bound, SeqOrUid::Value(n) if n.get() > count);
    set.0.as_ref().iter().any(|sequence| match sequence {
        Sequence::Single(one) => past(one),
        Sequence::Range(a, b) => past(a) || past(b),
    })
}

/// Whether `set` holds `value`, with `*` standing for `max`.
pub(super) fn contains(set: &SequenceSet, value: u32, max: u32) -> bool {
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
/// `From` whatever the list names, and the added header lines it names.
pub(super) fn header_fields(
    items: &MacroOrMessageDataItemNames<'static>,
) -> Option<Section<'static>> {
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

/// `From`, then each added header field `section` names, then the blank line.
fn served_fields(message: &FakeMessage, section: &Section<'static>) -> String {
    let mut data = format!("From: {}\r\n", message.sender_header());
    if let Section::HeaderFields(_, names) = section {
        for name in names.as_ref() {
            let name = String::from_utf8_lossy(name.as_ref());
            if let Some(value) = message.header_value(&name) {
                write!(data, "{name}: {value}\r\n").unwrap();
            }
        }
    }

    data + "\r\n"
}

/// A section of the body the client asked for, and the octets it asked for.
pub(super) struct PartAsked {
    pub(super) section: Asking,
    pub(super) partial: Option<(u32, NonZeroU32)>,
}

#[derive(PartialEq, Eq)]
pub(super) enum Asking {
    /// `BODY[HEADER]`, the whole Message's header.
    Header,
    /// `BODY[1.2]`.
    Part(Vec1<NonZeroU32>),
    /// `BODY[1.2.MIME]`.
    Mime(Vec1<NonZeroU32>),
}

/// The items of a `FETCH` the fake answers only when asked.
pub(super) struct Asked {
    header_fields: Option<Section<'static>>,
    parts: Vec<PartAsked>,
    /// `BODY[]`, the whole Message.
    whole: bool,
}

pub(super) fn whole_body(items: &MacroOrMessageDataItemNames<'static>) -> bool {
    let MacroOrMessageDataItemNames::MessageDataItemNames(names) = items else {
        return false;
    };

    names.iter().any(|name| {
        matches!(
            name,
            MessageDataItemName::BodyExt {
                section: None,
                partial: None,
                ..
            }
        )
    })
}

pub(super) fn body_parts(items: &MacroOrMessageDataItemNames<'static>) -> Vec<PartAsked> {
    let MacroOrMessageDataItemNames::MessageDataItemNames(names) = items else {
        return Vec::new();
    };

    names
        .iter()
        .filter_map(|name| {
            let MessageDataItemName::BodyExt {
                section: Some(section),
                partial,
                ..
            } = name
            else {
                return None;
            };

            let section = match section {
                Section::Part(Part(path)) => Asking::Part(path.clone()),
                Section::Mime(Part(path)) => Asking::Mime(path.clone()),
                Section::Header(None) => Asking::Header,
                _ => return None,
            };

            Some(PartAsked {
                section,
                partial: *partial,
            })
        })
        .collect()
}

/// The octets asked for, empty for a part the body does not have.
pub(super) fn body_part_item(message: &FakeMessage, asked: &PartAsked) -> MessageDataItem<'static> {
    let body = &message.body;
    let (content, section) = match &asked.section {
        Asking::Header => (
            body.header(&message.whole_headers()).into_bytes(),
            Section::Header(None),
        ),
        Asking::Part(path) => (
            body.part(path.as_ref()).unwrap_or_default(),
            Section::Part(Part(path.clone())),
        ),
        Asking::Mime(path) => (
            body.mime(path.as_ref()).unwrap_or_default().into_bytes(),
            Section::Mime(Part(path.clone())),
        ),
    };

    let (start, len) = asked.partial.map_or((0, content.len()), |(start, len)| {
        (
            usize::try_from(start).unwrap(),
            usize::try_from(len.get()).unwrap(),
        )
    });
    let served = content.get(start..).unwrap_or_default();
    let served = &served[..len.min(served.len())];
    MessageDataItem::BodyExt {
        section: Some(section),
        origin: asked.partial.map(|(start, _)| start),
        data: NString(Some(body_string(served))),
    }
}

/// A short part goes quoted, as a server may send it, unless it holds a
/// `"` or `\`: imap-proto reads a quoted string with its escapes left in.
fn body_string(served: &[u8]) -> IString<'static> {
    if served.iter().any(|b| matches!(b, b'"' | b'\\')) {
        IString::Literal(Literal::try_from(served.to_vec()).unwrap())
    } else {
        IString::try_from(served.to_vec()).unwrap()
    }
}

pub(super) fn whole_body_item(message: &FakeMessage) -> MessageDataItem<'static> {
    let whole = IString::try_from(message.body.message(&message.whole_headers())).unwrap();
    MessageDataItem::BodyExt {
        section: None,
        origin: None,
        data: NString(Some(whole)),
    }
}

/// An envelope's address list of each `(name, address)`.
pub(super) fn addresses(listed: &[(&str, &str)]) -> Vec<Address<'static>> {
    let nstring = |s: &str| NString(Some(IString::try_from(s.to_owned()).unwrap()));
    listed
        .iter()
        .map(|(name, address)| {
            let (mailbox, host) = address.split_once('@').unwrap();
            Address {
                name: nstring(name),
                adl: NString(None),
                mailbox: nstring(mailbox),
                host: nstring(host),
            }
        })
        .collect()
}

/// Every item whatever the client asked for, as a client must cope with extras.
pub(super) fn fetch(seq: u32, stored: &Stored, asked: &Asked) -> Data<'static> {
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
        to: addresses(&message.to),
        cc: addresses(&message.cc),
        bcc: vec![],
        in_reply_to: message
            .header_value("In-Reply-To")
            .map_or(NString(None), nstring),
        message_id: message.message_id.map_or(NString(None), nstring),
    };

    let received = chrono::DateTime::parse_from_rfc3339(&message.internal_date).unwrap();
    let mut items = vec![
        MessageDataItem::Uid(NonZeroU32::new(stored.uid).unwrap()),
        MessageDataItem::Flags(flags(message.seen, message.answered)),
        MessageDataItem::InternalDate(DateTime::try_from(received).unwrap()),
        MessageDataItem::Envelope(envelope),
        MessageDataItem::ModSeq(NonZeroU64::new(stored.modseq).unwrap()),
        MessageDataItem::BodyStructure(message.body.structure()),
    ];
    for part in &asked.parts {
        items.push(body_part_item(message, part));
    }

    if asked.whole {
        items.push(whole_body_item(message));
    }

    if let Some(section) = &asked.header_fields {
        items.push(MessageDataItem::BodyExt {
            section: Some(section.clone()),
            origin: None,
            data: nstring(&served_fields(message, section)),
        });
    }

    Data::Fetch {
        seq: NonZeroU32::new(seq).unwrap(),
        items: Vec1::try_from(items).unwrap(),
    }
}
