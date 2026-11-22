use lettre::message::Mailbox;
use mail_parser::{Address, MessageParser};

use crate::{
    compose::recipients::Recipients, domain::answering::Answering, domain::message::Message,
    render::body_text, render::header_date, render::html_preview::Links,
};

/// Columns HTML is laid out in before it is quoted, so that with `> ` a
/// line stays within 78.
const QUOTE_WIDTH: usize = 76;

/// The Draft a Reply to `original` opens on, from its body `raw`, and what
/// puts it in the Thread. Reply All copies everyone else the original went
/// to but the `account`, whose `signature` goes above the quote.
pub fn reply(
    raw: &[u8],
    original: &Message,
    account: &str,
    recipients: Recipients,
    signature: Option<&str>,
) -> (String, Answering) {
    let answering = |in_reply_to, references| Answering {
        in_reply_to,
        references,
        original: original.on_server(),
        thread: original.thread,
    };

    let Some(message) = MessageParser::default().parse(raw) else {
        let text = draft(&[], &[], &subject(&original.subject), signature, "");
        return (text, answering(None, None));
    };

    // A Reply to the Account's own Message goes on to whom that one went.
    let to = if original.from_me {
        mailboxes([message.to()])
    } else {
        mailboxes([message.reply_to().or_else(|| message.from())])
    };

    let mut cc: Vec<Mailbox> = Vec::new();
    for mailbox in mailboxes([message.to(), message.cc()])
        .into_iter()
        .filter(|_| recipients == Recipients::All)
    {
        let address = mailbox.email.to_string();
        let taken = |m: &Mailbox| m.email.to_string().eq_ignore_ascii_case(&address);
        if !address.eq_ignore_ascii_case(account) && !to.iter().any(taken) && !cc.iter().any(taken)
        {
            cc.push(mailbox);
        }
    }

    let id = message.message_id().map(|id| format!("<{id}>"));
    let earlier = message
        .references()
        .as_text_list()
        .or_else(|| message.in_reply_to().as_text_list())
        .unwrap_or_default();
    let references: Vec<String> = earlier
        .iter()
        .map(|id| format!("<{id}>"))
        .chain(id.clone())
        .collect();
    let references = (!references.is_empty()).then(|| references.join(" "));

    let said = format!(
        "On {}, {} wrote:\n{}",
        header_date::date(&original.received).as_str(),
        original.sender,
        quote(&body_text::plain(&message, QUOTE_WIDTH, Links::Inline))
    );
    let subject = subject(message.subject().unwrap_or(&original.subject));
    (
        draft(&to, &cc, &subject, signature, &said),
        answering(id, references),
    )
}

/// The file: its headers, then a blank line to write in before the
/// `signature` and `said`.
fn draft(
    to: &[Mailbox],
    cc: &[Mailbox],
    subject: &str,
    signature: Option<&str>,
    said: &str,
) -> String {
    let list = |mailboxes: &[Mailbox]| {
        let each: Vec<String> = mailboxes.iter().map(ToString::to_string).collect();
        each.join(", ")
    };

    let signed = signature.map_or_else(String::new, |signature| format!("{signature}\n"));
    format!(
        "To: {}\nCc: {}\nSubject: {subject}\n\n\n{signed}{said}",
        list(to),
        list(cc)
    )
}

/// `Re: ` once, however often the Thread was answered.
fn subject(original: &str) -> String {
    let answered = original
        .get(..3)
        .is_some_and(|re| re.eq_ignore_ascii_case("re:"));
    if answered {
        original.to_owned()
    } else {
        format!("Re: {original}")
    }
}

/// Each address of `headers` that parses, in order.
pub fn mailboxes<'a>(headers: impl IntoIterator<Item = Option<&'a Address<'a>>>) -> Vec<Mailbox> {
    headers
        .into_iter()
        .flatten()
        .flat_map(Address::iter)
        .filter_map(|addr| {
            let email = addr.address()?.parse().ok()?;
            Some(Mailbox::new(addr.name().map(str::to_owned), email))
        })
        .collect()
}

/// `text` with each line behind `> `, or behind `>` alone where it is
/// empty or quoted already.
fn quote(text: &str) -> String {
    text.trim_end()
        .lines()
        .fold(String::new(), |mut quoted, line| {
            let line = line.trim_end();
            quoted.push('>');
            if !line.is_empty() && !line.starts_with('>') {
                quoted.push(' ');
            }

            quoted.push_str(line);
            quoted.push('\n');
            quoted
        })
}
