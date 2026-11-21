use async_imap::{
    Session,
    imap_proto::{Address, AttributeValue, Envelope, MessageSection, Response, SectionPath},
};
use chrono::DateTime;

use super::{Error, Pushed, connect::Transport, tagged, text_part};
use crate::{
    body_key::body_key,
    correspondence::{self, Correspondence, Person},
    header,
    message::Message,
    message_flags::MessageFlags,
    new_message::NewMessage,
};

/// Messages between two progress reports, each a redraw.
const PROGRESS_EVERY: u32 = 50;

/// As `INTERNALDATE` is sent (RFC 3501 9).
const INTERNAL_DATE: &str = "%d-%b-%Y %H:%M:%S %z";

/// The rows a `FETCH` of the list items answers; `progress` gets how many
/// have come so far, now and then. What else the server announces on the
/// way goes to `pushed`, in order.
pub async fn fetch(
    session: &mut Session<Box<dyn Transport>>,
    command: String,
    (uid_validity, email): (u32, &str),
    (pushed, progress): (&mut Vec<Pushed>, impl Fn(u32)),
) -> Result<Vec<NewMessage>, Error> {
    let mut messages = Vec::new();
    let mut received = 0;
    tagged::run(session, command, |response| {
        let row = match response {
            Response::Fetch(_, attributes) if envelope(attributes).is_some() => {
                message(attributes, (uid_validity, email))
            }
            _ => {
                pushed.extend(Pushed::from_response(response));
                return;
            }
        };

        messages.extend(row);
        received += 1;
        if received % PROGRESS_EVERY == 0 {
            progress(received);
        }
    })
    .await?;
    Ok(messages)
}

/// `None` when the server left out the UID; `email` is the Account's.
fn message(
    attributes: &[AttributeValue<'_>],
    (uid_validity, email): (u32, &str),
) -> Option<NewMessage> {
    let uid = attributes.iter().find_map(|a| match a {
        AttributeValue::Uid(uid) => Some(*uid),
        _ => None,
    })?;
    let gmail_id = attributes.iter().find_map(|a| match a {
        AttributeValue::GmailMsgId(id) => Some(*id),
        _ => None,
    });
    let thread = attributes.iter().find_map(|a| match a {
        AttributeValue::GmailThrId(id) => Some(*id),
        _ => None,
    });
    let received = attributes
        .iter()
        .find_map(|a| match a {
            AttributeValue::InternalDate(date) => {
                DateTime::parse_from_str(date, INTERNAL_DATE).ok()
            }
            _ => None,
        })
        .unwrap_or_default();
    let flags = MessageFlags::from_names(
        attributes
            .iter()
            .flat_map(|a| match a {
                AttributeValue::Flags(flags) => flags.as_slice(),
                _ => &[],
            })
            .map(|f| &**f),
    );

    let envelope = envelope(attributes);
    let row = Message {
        uid,
        body_key: body_key(gmail_id, uid_validity, uid),
        received,
        unread: flags.unread,
        answered: flags.answered,
        sender: envelope
            .and_then(sender)
            .unwrap_or_else(|| raw_from(attributes)),
        from: from_line(envelope, attributes),
        to: envelope.map_or_else(String::new, |e| shown(e.to.iter().chain(&e.cc).flatten())),
        subject: subject(envelope),
        snippet: String::new(),
        thread,
        from_me: envelope.is_some_and(|e| from(e, email)),
        folder: None,
        account: 0,
    };

    let structure = attributes.iter().find_map(|a| match a {
        AttributeValue::BodyStructure(structure) => Some(structure),
        _ => None,
    });
    Some(NewMessage {
        correspondence: correspondence(envelope, &row),
        row,
        text_part: structure.and_then(text_part::find),
    })
}

fn subject(envelope: Option<&Envelope<'_>>) -> String {
    envelope
        .and_then(|e| e.subject.as_deref())
        .map(header::decode)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "(no subject)".to_owned())
}

/// Under its `Message-ID`, else under the body key of `row`.
fn correspondence(envelope: Option<&Envelope<'_>>, row: &Message) -> Correspondence {
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
fn from_line(envelope: Option<&Envelope<'_>>, attributes: &[AttributeValue<'_>]) -> String {
    envelope
        .and_then(|e| e.from.as_ref()?.first())
        .and_then(one)
        .unwrap_or_else(|| raw_from(attributes))
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

/// The `From` header's value as sent, decoded to one line.
fn raw_from(attributes: &[AttributeValue<'_>]) -> String {
    let header = attributes
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
        .unwrap_or_default();
    let value = header
        .iter()
        .position(|&b| b == b':')
        .map_or(&[][..], |colon| &header[colon + 1..]);
    header::decode(value).trim().to_owned()
}
