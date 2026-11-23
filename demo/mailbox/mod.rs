//! The demo mailbox: invented people and companies only, so no frame of the
//! landing page shows anyone's real mail or brand. Dates count back from the
//! start of the run, so the List always reads as this week's mail.

mod personal;
mod work;

use chrono::{DateTime, Duration, Local, NaiveTime, SecondsFormat, TimeZone, Timelike};

pub use personal::personal;
pub use work::work;

use crate::support::{fake_body::FakeBody, fake_imap::FakeMessage};

const SAM: &str = "Sam Okafor";

/// One of Sam's Accounts, by its address: the mail it gets and sends.
struct Account(&'static str);

impl Account {
    /// A plain-text Message from `name` at `address` to Sam.
    fn mail(
        &self,
        (name, address): (&'static str, &'static str),
        subject: &str,
        when: DateTime<Local>,
        body: &str,
    ) -> FakeMessage {
        FakeMessage {
            from_address: address,
            ..FakeMessage::new(name, subject, &rfc3339(when))
        }
        .to(SAM, self.0)
        .body(FakeBody::plain(body))
    }

    /// A plain-text Message Sam sent to `name` at `address`.
    fn sent(
        &self,
        (name, address): (&'static str, &'static str),
        subject: &str,
        when: DateTime<Local>,
        body: &str,
    ) -> FakeMessage {
        FakeMessage {
            from_address: self.0,
            ..FakeMessage::new(SAM, subject, &rfc3339(when))
        }
        .to(name, address)
        .body(FakeBody::plain(body))
    }
}

/// `days` before today at `time` ("HH:MM"), local.
fn at(days: i64, time: &str) -> DateTime<Local> {
    let time = NaiveTime::parse_from_str(time, "%H:%M").unwrap();
    let day = Local::now().date_naive() - Duration::days(days);
    Local.from_local_datetime(&day.and_time(time)).unwrap()
}

/// `minutes` before the start of the run: today's mail is never later than
/// now, whatever the hour the demo runs at.
fn ago(minutes: i64) -> DateTime<Local> {
    (Local::now() - Duration::minutes(minutes))
        .with_nanosecond(0)
        .unwrap()
}

/// The fake server takes whole seconds only.
fn rfc3339(when: DateTime<Local>) -> String {
    when.to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// One of the HTML mails of `demo/mail`, dated `when`.
fn html(raw: &str, when: DateTime<Local>) -> FakeMessage {
    FakeMessage::eml(raw.replace("@DATE@", &when.to_rfc2822()).as_bytes())
}

/// `message` with `files` after its text, each `(type, subtype, name, kilobytes)`.
fn attach(
    message: FakeMessage,
    files: &[(&'static str, &'static str, &str, usize)],
) -> FakeMessage {
    let mut parts = vec![message.body.clone()];
    parts.extend(files.iter().map(|&(media_type, subtype, name, kb)| {
        let file = FakeBody::file(media_type, subtype).lines(kb * 1024 / 57);
        if name.is_ascii() {
            file.disposition_param("filename", name)
        } else {
            file.disposition_param("filename*", &rfc2231(name))
        }
    }));
    message.body(FakeBody::multipart("mixed", parts))
}

/// `name` as an RFC 2231 value, for a file name past ASCII.
fn rfc2231(name: &str) -> String {
    let escaped: String = name
        .bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'-' | b'_' => {
                char::from(b).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect();
    format!("utf-8''{escaped}")
}
