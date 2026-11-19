use lettre::{
    Message,
    message::{Mailbox, Mailboxes, header::ContentType},
};

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
    /// non-ASCII headers RFC 2047-encoded.
    pub fn message(&self, from: Mailbox, id: String) -> Result<Message, String> {
        let mut message = Message::builder()
            .from(from)
            .subject(&self.subject)
            .date_now()
            .message_id(Some(id));
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
