//! A Message as the fake server holds it, built up by a test.

use std::fmt::Write;

use mail_parser::{Addr, MessageParser};

use super::super::fake_body::FakeBody;
use super::EMAIL;

#[derive(Clone)]
pub struct FakeMessage {
    pub from_name: Option<&'static str>,
    pub from_address: &'static str,
    /// A raw `From` header the server could not parse: the envelope then has no `From`.
    pub unparsable_from: Option<&'static str>,
    /// Raw header text, so a test can send RFC 2047 encoded words.
    pub subject: String,
    /// RFC 3339.
    pub internal_date: String,
    pub seen: bool,
    pub body: FakeBody,
    /// Messages sharing it share an `X-GM-THRID`; `None` starts a Thread of its own.
    pub thread: Option<u64>,
    /// More header lines of the whole Message, each ending in CRLF.
    pub headers: String,
    pub answered: bool,
    /// Each `(name, address)` of its envelope's `To`, then of its `Cc`.
    pub to: Vec<(&'static str, &'static str)>,
    pub cc: Vec<(&'static str, &'static str)>,
    pub message_id: Option<&'static str>,
}

impl FakeMessage {
    pub fn new(from_name: &'static str, subject: &str, internal_date: &str) -> Self {
        Self {
            from_name: Some(from_name),
            from_address: "someone@example.com",
            unparsable_from: None,
            subject: subject.to_owned(),
            internal_date: internal_date.to_owned(),
            seen: true,
            body: FakeBody::plain(""),
            thread: None,
            headers: String::new(),
            answered: false,
            to: Vec::new(),
            cc: Vec::new(),
            message_id: None,
        }
    }

    /// The whole `.eml` `raw`, under its own From, To, Subject and Date.
    pub fn eml(raw: &[u8]) -> Self {
        let leak = |s: &str| -> &'static str { s.to_owned().leak() };
        let addr = |a: &Addr<'_>| (leak(a.name().unwrap_or("")), leak(a.address().unwrap()));
        let parsed = MessageParser::default().parse(raw).unwrap();
        let (from_name, from_address) = addr(parsed.from().unwrap().first().unwrap());
        let subject = parsed.header_raw("Subject").unwrap_or("");
        let mut message = Self::new(
            from_name,
            subject.replace("\r\n", "").replace('\n', "").trim(),
            &parsed.date().unwrap().to_rfc3339(),
        )
        .body(FakeBody::eml(raw));
        message.from_address = from_address;
        message.to = parsed
            .to()
            .map(|to| to.iter().map(addr).collect())
            .unwrap_or_default();
        message
    }

    /// Sent to `name` at `address`.
    pub fn to(mut self, name: &'static str, address: &'static str) -> Self {
        self.to.push((name, address));
        self
    }

    pub fn cc(mut self, name: &'static str, address: &'static str) -> Self {
        self.cc.push((name, address));
        self
    }

    pub const fn message_id(mut self, id: &'static str) -> Self {
        self.message_id = Some(id);
        self
    }

    /// Adds `name: value` to the whole Message's header.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        write!(self.headers, "{name}: {value}\r\n").unwrap();
        self
    }

    pub fn body(mut self, body: FakeBody) -> Self {
        self.body = body;
        self
    }

    pub const fn unread(mut self) -> Self {
        self.seen = false;
        self
    }

    pub const fn answered(mut self) -> Self {
        self.answered = true;
        self
    }

    pub const fn in_thread(mut self, thread: u64) -> Self {
        self.thread = Some(thread);
        self
    }

    /// Sent from the Account's own address.
    pub const fn sent_by_me(mut self) -> Self {
        self.from_address = EMAIL;
        self
    }

    pub const fn unparsable_from(mut self, raw: &'static str) -> Self {
        self.unparsable_from = Some(raw);
        self
    }

    /// The header lines before the body's own MIME ones, each ending in CRLF.
    pub(super) fn whole_headers(&self) -> String {
        format!(
            "From: {}\r\nSubject: {}\r\n{}",
            self.sender_header(),
            self.subject,
            self.headers
        )
    }

    /// The added header field `name`, its case ignored, as written: folds
    /// kept, no final CRLF.
    pub(super) fn header_value(&self, name: &str) -> Option<&str> {
        let mut fields = self.headers.split_inclusive("\r\n").peekable();
        let mut start = 0;
        while let Some(line) = fields.next() {
            let mut end = start + line.len();
            while fields
                .peek()
                .is_some_and(|next| next.starts_with([' ', '\t']))
            {
                end += fields.next().unwrap().len();
            }

            let field = self.headers.get(start..end - 2)?;
            let (field_name, value) = field.split_once(':')?;
            if field_name.eq_ignore_ascii_case(name) {
                return Some(value.trim_start());
            }

            start = end;
        }

        None
    }

    /// The `From` header's value as the Message carries it.
    pub(super) fn sender_header(&self) -> String {
        match (self.unparsable_from, self.from_name) {
            (Some(raw), _) => raw.to_owned(),
            (None, Some(name)) => format!("{name} <{}>", self.from_address),
            (None, None) => self.from_address.to_owned(),
        }
    }
}
