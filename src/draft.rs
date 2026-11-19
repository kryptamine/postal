use lettre::{
    Message,
    message::{Mailbox, Mailboxes, header::ContentType},
};

use crate::answering::Answering;

/// A Draft's file as read: `To`, `Cc` and `Subject` lines, a blank line,
/// then the body. Postal adds every other header itself.
pub struct Draft {
    pub to: Mailboxes,
    pub cc: Mailboxes,
    pub subject: String,
    pub body: String,
}

impl Draft {
    /// The error says what to fix in the file.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.replace("\r\n", "\n");
        let (head, body) = text.split_once("\n\n").unwrap_or((&text, ""));
        let (mut to, mut cc, mut subject) = (None, None, None);
        for (name, value) in fields(head)? {
            let slot = match name.to_ascii_lowercase().as_str() {
                "to" => &mut to,
                "cc" => &mut cc,
                "subject" => &mut subject,
                _ => return Err(format!("unknown header `{name}`: only To, Cc and Subject")),
            };

            if slot.replace(value).is_some() {
                return Err(format!("`{name}` is given twice"));
            }
        }

        let to = to.filter(|to| !to.is_empty()).ok_or("`To` is empty")?;
        Ok(Self {
            to: addresses("To", &to)?,
            cc: match cc.filter(|cc| !cc.is_empty()) {
                Some(cc) => addresses("Cc", &cc)?,
                None => Mailboxes::new(),
            },
            subject: subject.unwrap_or_default(),
            body: body.to_owned(),
        })
    }

    /// The Message as sent: plain UTF-8 text, its lines as written, and
    /// non-ASCII headers RFC 2047-encoded; in its Thread when `answering`.
    pub fn message(
        &self,
        from: Mailbox,
        id: String,
        answering: Option<&Answering>,
    ) -> Result<Message, String> {
        let mut message = Message::builder()
            .from(from)
            .subject(&self.subject)
            .date_now()
            .message_id(Some(id));
        if let Some(in_reply_to) = answering.and_then(|a| a.in_reply_to.clone()) {
            message = message.in_reply_to(in_reply_to);
        }

        if let Some(references) = answering.and_then(|a| a.references.clone()) {
            message = message.references(references);
        }

        for to in self.to.iter() {
            message = message.to(to.clone());
        }

        for cc in self.cc.iter() {
            message = message.cc(cc.clone());
        }

        message
            .header(ContentType::TEXT_PLAIN)
            .body(self.body.clone())
            .map_err(|error| error.to_string())
    }
}

/// Header `name` as written, from a Draft that need not parse.
pub fn header(text: &str, name: &str) -> Option<String> {
    let text = text.replace("\r\n", "\n");
    let head = text.split_once("\n\n").map_or(&*text, |(head, _)| head);
    let fields = fields(head).ok()?;
    let (_, value) = fields
        .into_iter()
        .find(|(field, _)| field.eq_ignore_ascii_case(name))?;
    Some(value)
}

/// Each header's name and value, a folded line joined to the one before.
fn fields(head: &str) -> Result<Vec<(&str, String)>, String> {
    let mut fields: Vec<(&str, String)> = Vec::new();
    for line in head.lines() {
        if line.starts_with([' ', '\t']) {
            let Some((_, value)) = fields.last_mut() else {
                return Err(format!("not a header: `{}`", line.trim()));
            };

            value.push(' ');
            value.push_str(line.trim());
            continue;
        }

        let Some((name, value)) = line.split_once(':') else {
            return Err(format!("not a header: `{line}`"));
        };

        fields.push((name.trim(), value.trim().to_owned()));
    }

    Ok(fields)
}

fn addresses(name: &str, value: &str) -> Result<Mailboxes, String> {
    value
        .parse()
        .map_err(|_bad| format!("`{name}` holds a bad address: {value}"))
}
