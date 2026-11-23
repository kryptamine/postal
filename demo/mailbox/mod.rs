//! The demo mailbox: invented people and companies only, so no frame of the
//! landing page shows anyone's real mail or brand. Dates count back from the
//! start of the run, so the List always reads as this week's mail.

mod personal;
mod work;

use chrono::{Duration, Local, NaiveTime, SecondsFormat, TimeZone};

pub use personal::personal;
pub use work::work;

use crate::support::{fake_body::FakeBody, fake_imap::FakeMessage};

/// `days` before today at `time` ("HH:MM"), local, as RFC 3339.
fn at(days: i64, time: &str) -> String {
    let time = NaiveTime::parse_from_str(time, "%H:%M").unwrap();
    let day = Local::now().date_naive() - Duration::days(days);
    let when = Local.from_local_datetime(&day.and_time(time)).unwrap();
    when.to_rfc3339()
}

/// `minutes` before the start of the run, as RFC 3339: today's mail is
/// never later than now, whatever the hour the demo runs at.
fn ago(minutes: i64) -> String {
    (Local::now() - Duration::minutes(minutes)).to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// A plain-text Message from `name` at `address`, sent to the work Account.
fn mail(
    name: &'static str,
    address: &'static str,
    subject: &str,
    when: &str,
    body: &str,
) -> FakeMessage {
    FakeMessage {
        from_address: address,
        ..FakeMessage::new(name, subject, when)
    }
    .to("Sam Okafor", crate::support::fake_imap::EMAIL)
    .body(FakeBody::plain(body))
}

/// One of the HTML mails of `demo/mail`, dated `when`.
fn html(raw: &str, when: &str) -> FakeMessage {
    let date = chrono::DateTime::parse_from_rfc3339(when)
        .unwrap()
        .to_rfc2822();
    FakeMessage::eml(raw.replace("@DATE@", &date).as_bytes())
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
