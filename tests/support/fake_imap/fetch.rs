//! What a `FETCH` asks for, and the items the fake answers it with.

use std::num::{NonZeroU32, NonZeroU64};

use imap_next::{
    imap_types::{
        command::{CommandBody, FetchModifier},
        core::{IString, NString, Tag, Vec1},
        datetime::DateTime,
        envelope::{Address, Envelope},
        fetch::{MacroOrMessageDataItemNames, MessageDataItem, MessageDataItemName, Part, Section},
        response::{Data, Status},
        sequence::{SeqOrUid, Sequence, SequenceSet},
    },
    server::Server,
};

use super::super::fake_body::FakeBody;
use super::{Folder, Inbox, Stored, flags, message::FakeMessage};

/// Enqueues what a `FETCH` asks for and returns its tagged status.
pub(super) fn answer_fetch(
    server: &mut Server,
    tag: Option<Tag<'static>>,
    body: CommandBody<'static>,
    (inbox, folder): (&Inbox, &Folder),
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
        from_header: header_fields(&macro_or_item_names),
        part: body_part(&macro_or_item_names),
        whole: whole_body(&macro_or_item_names),
    };

    let refused = fetched(folder, &sequence_set, uid, None)
        .any(|(_, stored)| inbox.refused.contains(&stored.uid));
    if asked.whole && refused {
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
/// only the `From` field whatever the list names.
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

/// A body part number the client asked for, and the octets it asked for.
pub(super) struct PartAsked {
    path: Vec1<NonZeroU32>,
    partial: Option<(u32, NonZeroU32)>,
}

/// The items of a `FETCH` the fake answers only when asked.
pub(super) struct Asked {
    from_header: Option<Section<'static>>,
    part: Option<PartAsked>,
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

pub(super) fn body_part(items: &MacroOrMessageDataItemNames<'static>) -> Option<PartAsked> {
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
pub(super) fn body_part_item(body: &FakeBody, asked: &PartAsked) -> MessageDataItem<'static> {
    let content = body.part(asked.path.as_ref()).unwrap_or_default();
    let (start, len) = asked.partial.map_or((0, content.len()), |(start, len)| {
        (
            usize::try_from(start).unwrap(),
            usize::try_from(len.get()).unwrap(),
        )
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

pub(super) fn whole_body_item(message: &FakeMessage) -> MessageDataItem<'static> {
    let headers = format!(
        "From: {}\r\nSubject: {}\r\n{}",
        message.sender_header(),
        message.subject,
        message.headers
    );
    let whole = IString::try_from(message.body.message(&headers)).unwrap();
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
        in_reply_to: NString(None),
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
    if let Some(asked) = &asked.part {
        items.push(body_part_item(&message.body, asked));
    }

    if asked.whole {
        items.push(whole_body_item(message));
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
