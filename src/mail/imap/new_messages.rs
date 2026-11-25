use async_imap::{
    Session,
    imap_proto::{Address, AttributeValue, Envelope, MessageSection, Response, SectionPath},
};
use chrono::{DateTime, FixedOffset};

use super::{
    Error, Pushed, attachments, attributes, connect::Transport, origin::Origin, tagged, text_part,
};
use crate::{
    domain::body_key::BodyKey,
    domain::correspondence::{self, Correspondence, Person},
    domain::message::Message,
    domain::message_flags::MessageFlags,
    domain::new_message::NewMessage,
    domain::root_id::RootId,
    domain::thread_id::ThreadId,
    mail::header,
};

/// Messages between two progress reports, each a redraw.
const PROGRESS_EVERY: u32 = 50;

/// As `INTERNALDATE` is sent (RFC 3501 9).
const INTERNAL_DATE: &str = "%d-%b-%Y %H:%M:%S %z";

/// The rows a `FETCH` of the list items answers; `progress` gets how many
/// have come so far, now and then.
pub async fn fetch(
    session: &mut Session<Box<dyn Transport>>,
    pushed: &mut Vec<Pushed>,
    command: String,
    origin: Origin<'_>,
    progress: impl Fn(u32),
) -> Result<Vec<NewMessage>, Error> {
    let mut messages = Vec::new();
    let mut received = 0;
    tagged::run(session, pushed, command, |response| {
        let Response::Fetch(_, attributes) = response else {
            return false;
        };

        if envelope(attributes).is_none() {
            return false;
        }

        messages.extend(message(attributes, origin));
        received += 1;
        if received % PROGRESS_EVERY == 0 {
            progress(received);
        }

        true
    })
    .await?;
    Ok(messages)
}

/// `None` when the server left out the UID.
fn message(attributes: &[AttributeValue<'_>], origin: Origin<'_>) -> Option<NewMessage> {
    let uid = attributes::uid(attributes)?;
    let gmail_id = attributes.iter().find_map(|a| match a {
        AttributeValue::GmailMsgId(id) => Some(*id),
        _ => None,
    });
    let thread = attributes.iter().find_map(|a| match a {
        AttributeValue::GmailThrId(id) => Some(ThreadId::new(*id)),
        _ => None,
    });
    let received = internal_date(attributes);
    let flags = attributes::flags(attributes).unwrap_or_else(|| MessageFlags::from_names([]));

    let envelope = envelope(attributes);
    let header = header_block(attributes);
    let structure = attributes::structure(attributes);
    let row = Message {
        uid,
        body_key: BodyKey::new(gmail_id, origin.uid_validity, uid.get()),
        received,
        unread: flags.unread,
        answered: flags.answered,
        sender: envelope
            .and_then(sender)
            .unwrap_or_else(|| raw_from(header)),
        from: from_line(envelope, header),
        to: envelope.map_or_else(String::new, |e| shown(e.to.iter().chain(&e.cc).flatten())),
        subject: subject(envelope),
        snippet: String::new(),
        attachments: structure.map(attachments::find).unwrap_or_default(),
        root: thread.is_none().then(|| root(header, envelope)).flatten(),
        thread,
        from_me: envelope.is_some_and(|e| from(e, origin.email)),
        folder: None,
        account: (),
    };

    Some(NewMessage {
        correspondence: correspondence(envelope, &row),
        row,
        text_part: structure.and_then(text_part::find),
    })
}

/// The first `Message-ID` of `References`, else of `In-Reply-To`, else
/// the Message's own; `None` when it names none.
fn root(header: &[u8], envelope: Option<&Envelope<'_>>) -> Option<RootId> {
    header::field(header, "References")
        .and_then(first_id)
        .or_else(|| envelope?.in_reply_to.as_deref().and_then(first_id))
        .or_else(|| envelope?.message_id.as_deref().and_then(first_id))
        .map(RootId::of)
}

/// The first `<id>` of `ids`, without its brackets.
fn first_id(ids: &[u8]) -> Option<&[u8]> {
    let open = ids.iter().position(|&b| b == b'<')?;
    let close = ids[open..].iter().position(|&b| b == b'>')?;
    Some(&ids[open + 1..open + close]).filter(|id| !id.is_empty())
}

fn internal_date(attributes: &[AttributeValue<'_>]) -> DateTime<FixedOffset> {
    attributes
        .iter()
        .find_map(|a| match a {
            AttributeValue::InternalDate(date) => {
                DateTime::parse_from_str(date, INTERNAL_DATE).ok()
            }
            _ => None,
        })
        .unwrap_or_default()
}

fn subject(envelope: Option<&Envelope<'_>>) -> String {
    envelope
        .and_then(|e| e.subject.as_deref())
        .map(header::decode)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "(no subject)".to_owned())
}

/// Under its `Message-ID`, else under the body key of `row`.
fn correspondence(envelope: Option<&Envelope<'_>>, row: &Message<()>) -> Correspondence {
    let listed = |addresses: Option<&Vec<Address<'_>>>| -> Vec<Person> {
        addresses.into_iter().flatten().filter_map(person).collect()
    };

    let id = envelope.and_then(|e| e.message_id.as_deref());
    let id = id
        .map(String::from_utf8_lossy)
        .filter(|id| !id.trim().is_empty());
    let mut to = listed(envelope.and_then(|e| e.to.as_ref()));
    to.extend(listed(envelope.and_then(|e| e.cc.as_ref())));
    Correspondence {
        message: id.map_or_else(
            || format!("key:{}", row.body_key),
            |id| correspondence::key(&id),
        ),
        at: row.received.to_utc(),
        from: listed(envelope.and_then(|e| e.from.as_ref())),
        to,
        sent: row.from_me,
    }
}

/// Its address and decoded display name, if it has one; `None` for a
/// group's start or end, which has no host.
fn parts(address: &Address<'_>) -> Option<(String, Option<String>)> {
    let mailbox = address.mailbox.as_deref().filter(|m| !m.is_empty())?;
    let host = address.host.as_deref().filter(|h| !h.is_empty())?;
    let email = format!(
        "{}@{}",
        String::from_utf8_lossy(mailbox),
        String::from_utf8_lossy(host)
    );
    let name = address.name.as_deref().map(header::decode);
    Some((email, name.filter(|name| !name.trim().is_empty())))
}

fn person(address: &Address<'_>) -> Option<Person> {
    let (email, name) = parts(address)?;
    Some(Person::new(&email, name))
}

/// Whether the first `From` address is `email`; addresses compare without case.
fn from(envelope: &Envelope<'_>, email: &str) -> bool {
    envelope
        .from
        .as_ref()
        .and_then(|from| parts(from.first()?))
        .is_some_and(|(address, _)| address.eq_ignore_ascii_case(email))
}

fn envelope<'a>(attributes: &'a [AttributeValue<'_>]) -> Option<&'a Envelope<'a>> {
    attributes.iter().find_map(|a| match a {
        AttributeValue::Envelope(envelope) => Some(&**envelope),
        _ => None,
    })
}

/// The `From` display name, else the full address; `None` when the server
/// found no address in the header.
fn sender(envelope: &Envelope<'_>) -> Option<String> {
    let (email, name) = parts(envelope.from.as_ref()?.first()?)?;
    Some(name.unwrap_or(email))
}

/// The first `From` address as `one` shows it, else the header as sent.
fn from_line(envelope: Option<&Envelope<'_>>, header: &[u8]) -> String {
    envelope
        .and_then(|e| e.from.as_ref()?.first())
        .and_then(one)
        .unwrap_or_else(|| raw_from(header))
}

/// Each of `addresses` as `one` shows it, joined by commas.
fn shown<'a>(addresses: impl Iterator<Item = &'a Address<'a>>) -> String {
    addresses.filter_map(one).collect::<Vec<_>>().join(", ")
}

/// `Name <address>`, or the address alone when the name is none or only
/// repeats it.
fn one(address: &Address<'_>) -> Option<String> {
    let (email, name) = parts(address)?;
    Some(
        match name.filter(|name| !name.eq_ignore_ascii_case(&email)) {
            Some(name) => format!("{name} <{email}>"),
            None => email,
        },
    )
}

/// The header fields the list items asked for; empty when none came.
fn header_block<'a>(attributes: &'a [AttributeValue<'_>]) -> &'a [u8] {
    attributes
        .iter()
        .find_map(|a| match a {
            AttributeValue::BodySection {
                section: Some(SectionPath::Full(MessageSection::Header)),
                data: Some(header),
                ..
            }
            | AttributeValue::Rfc822Header(Some(header)) => Some(&**header),
            _ => None,
        })
        .unwrap_or_default()
}

/// The `From` header's value as sent, decoded to one line.
fn raw_from(header: &[u8]) -> String {
    header::decode(header::field(header, "From").unwrap_or_default())
        .trim()
        .to_owned()
}
