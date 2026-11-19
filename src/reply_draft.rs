use lettre::message::Mailbox;
use mail_parser::{Address, MessageParser, PartType};

use crate::{
    answering::Answering,
    html_preview::{self, Links},
    message::Message,
    preview_render::DATE,
};

/// Columns HTML is laid out in before it is quoted, so that with `> ` a
/// line stays within 78.
const QUOTE_WIDTH: usize = 76;

/// The Draft a Reply to `original` opens on, from its body `raw`, and what
/// puts it in the Thread. Reply All, with `all`, copies everyone else the
/// original went to but the `account`.
pub fn reply(raw: &[u8], original: &Message, account: &str, all: bool) -> (String, Answering) {
    let answering = |in_reply_to, references| Answering {
        in_reply_to,
        references,
        original: original.folder.clone().map(|folder| (folder, original.uid)),
    };

    let Some(message) = MessageParser::default().parse(raw) else {
        let text = draft(&[], &[], &subject(&original.subject), "");
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
        .filter(|_| all)
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
        original.received.format(DATE),
        original.sender,
        quote(&text(&message))
    );
    let subject = subject(message.subject().unwrap_or(&original.subject));
    (draft(&to, &cc, &subject, &said), answering(id, references))
}

/// The file: its headers, then a blank line to write in before `said`.
fn draft(to: &[Mailbox], cc: &[Mailbox], subject: &str, said: &str) -> String {
    let list = |mailboxes: &[Mailbox]| {
        let each: Vec<String> = mailboxes.iter().map(ToString::to_string).collect();
        each.join(", ")
    };

    format!(
        "To: {}\nCc: {}\nSubject: {subject}\n\n\n{said}",
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
fn mailboxes<'a>(headers: impl IntoIterator<Item = Option<&'a Address<'a>>>) -> Vec<Mailbox> {
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

/// The `text/plain` part, else the HTML one as the Text View shows it.
fn text(message: &mail_parser::Message<'_>) -> String {
    if let Some(PartType::Text(text)) = message.text_part(0).map(|p| &p.body) {
        return text.to_string();
    }

    let Some(PartType::Html(html)) = message.html_part(0).map(|p| &p.body) else {
        return String::new();
    };

    html_preview::render(html, QUOTE_WIDTH, Links::Inline).map_or_else(
        |_| String::new(),
        |lines| {
            let each: Vec<&str> = lines.iter().map(|line| line.text.trim_end()).collect();
            each.join("\n")
        },
    )
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
