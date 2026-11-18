use async_imap::{
    Session,
    imap_proto::{AttributeValue, Envelope, MessageSection, Response, SectionPath},
};
use chrono::DateTime;

use super::{Error, Pushed, connect::Transport, tagged, text_part};
use crate::{body_key::body_key, header, message::Message, new_message::NewMessage};

/// Messages between two progress reports, each a redraw.
const PROGRESS_EVERY: u32 = 50;

/// As `INTERNALDATE` is sent (RFC 3501 9).
const INTERNAL_DATE: &str = "%d-%b-%Y %H:%M:%S %z";

/// The rows a `FETCH` of the list items answers; `progress` gets how many
/// have come so far, now and then. What else the server announces on the
/// way goes to `pushed`, in order.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
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
    let unread = !attributes.iter().any(|a| match a {
        AttributeValue::Flags(flags) => flags.iter().any(|f| f.eq_ignore_ascii_case("\\Seen")),
        _ => false,
    });

    let envelope = envelope(attributes);
    let subject = envelope
        .and_then(|e| e.subject.as_deref())
        .map(header::decode)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "(no subject)".to_owned());
    let row = Message {
        uid,
        body_key: body_key(gmail_id, uid_validity, uid),
        received,
        unread,
        sender: envelope
            .and_then(sender)
            .unwrap_or_else(|| raw_from(attributes)),
        subject,
        snippet: String::new(),
        thread,
        from_me: envelope.is_some_and(|e| from(e, email)),
        elsewhere: false,
    };

    let structure = attributes.iter().find_map(|a| match a {
        AttributeValue::BodyStructure(structure) => Some(structure),
        _ => None,
    });
    Some(NewMessage {
        row,
        text_part: structure.and_then(text_part::find),
    })
}

/// Whether the first `From` address is `email`; addresses compare without case.
fn from(envelope: &Envelope<'_>, email: &str) -> bool {
    let Some(from) = envelope.from.as_ref().and_then(|f| f.first()) else {
        return false;
    };

    let part = |p: Option<&[u8]>| String::from_utf8_lossy(p.unwrap_or_default()).into_owned();
    let address = format!(
        "{}@{}",
        part(from.mailbox.as_deref()),
        part(from.host.as_deref())
    );
    address.eq_ignore_ascii_case(email)
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
    let from = envelope.from.as_ref()?.first()?;
    let mailbox = from.mailbox.as_deref().filter(|m| !m.is_empty())?;
    let host = from.host.as_deref().filter(|h| !h.is_empty())?;
    if let Some(name) = from.name.as_deref().map(header::decode)
        && !name.trim().is_empty()
    {
        return Some(name);
    }

    Some(format!(
        "{}@{}",
        String::from_utf8_lossy(mailbox),
        String::from_utf8_lossy(host)
    ))
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
