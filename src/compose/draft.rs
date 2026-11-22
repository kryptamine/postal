use lettre::{
    Message,
    message::{Mailbox, Mailboxes, MultiPart, SinglePart, header::ContentType},
};

use crate::{
    compose::attach::{self, Attach},
    domain::answering::Answering,
};

/// A Draft's file as read: `To`, `Cc`, `Subject` and `Attach` lines, a
/// blank line, then the body. Postal adds every other header itself.
pub struct Draft {
    pub to: Mailboxes,
    pub cc: Mailboxes,
    pub subject: String,
    pub attached: Vec<Attach>,
    pub body: String,
}

impl Draft {
    /// The error says what to fix in the file. The files attached are
    /// not read: `attachable` checks them.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.replace("\r\n", "\n");
        let (head, body) = text.split_once("\n\n").unwrap_or((&text, ""));
        let (mut to, mut cc, mut subject, mut attached) = (None, None, None, Vec::new());
        for (name, value) in fields(head)? {
            let slot = match name.to_ascii_lowercase().as_str() {
                "to" => &mut to,
                "cc" => &mut cc,
                "subject" => &mut subject,
                "attach" => {
                    if !value.is_empty() {
                        attached.push(Attach { written: value });
                    }

                    continue;
                }
                _ => {
                    return Err(format!(
                        "unknown header `{name}`: only To, Cc, Subject and Attach"
                    ));
                }
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
            attached,
            body: body.to_owned(),
        })
    }

    /// Fails unless every file attached opens, within Gmail's limit
    /// together.
    pub fn attachable(&self) -> Result<(), String> {
        let mut total = 0;
        for attach in &self.attached {
            total += attach.size()?;
        }

        if total > attach::LIMIT {
            let mb = attach::LIMIT / 1024 / 1024;
            return Err(format!("the files attached are over {mb} MB together"));
        }

        Ok(())
    }

    /// The Message as sent: plain UTF-8 text, its lines as written, then
    /// each file attached, read now; non-ASCII headers RFC 2047-encoded; in
    /// its Thread when `answering`.
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

        let built = if self.attached.is_empty() {
            message
                .header(ContentType::TEXT_PLAIN)
                .body(self.body.clone())
        } else {
            self.attachable()?;
            let text = SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(self.body.clone());
            let mut mixed = MultiPart::mixed().singlepart(text);
            for attach in &self.attached {
                mixed = mixed.singlepart(attach.part()?);
            }

            message.multipart(mixed)
        };

        built.map_err(|error| error.to_string())
    }
}

/// Header `name` as written, from a Draft that need not parse.
pub fn header(text: &str, name: &str) -> Option<String> {
    headers(text, name).into_iter().next()
}

/// Every header `name` as written, in order, from a Draft that need not
/// parse; none when its head does not.
pub fn headers(text: &str, name: &str) -> Vec<String> {
    let text = text.replace("\r\n", "\n");
    let head = text.split_once("\n\n").map_or(&*text, |(head, _)| head);
    let fields = fields(head).unwrap_or_default();
    fields
        .into_iter()
        .filter(|(field, _)| field.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
        .collect()
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
