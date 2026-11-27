//! Forward with `F` opens `$EDITOR` on a Draft holding the selected
//! Message below a forwarded-message header, its attachments on `Attach:`
//! lines; the fake SMTP server receives it as a new Thread.

use crate::support;

use std::{fs, path::PathBuf};

use mail_parser::{MessageParser, MimeHeaders};
use support::{
    editor::{preview_newest, saved},
    fake_body::FakeBody,
    fake_imap::{EMAIL, FakeImap, FakeMessage, Outage, Stall},
    fake_oauth,
    fake_smtp::{FakeSmtp, Received},
    home::Home,
};
use termlens::{Key, Terminal};

const DATE: &str = "2024-11-18T09:00:00Z";

/// How the forwarded-message header names `DATE` in UTC.
const SHOWN_DATE: &str = "Mon, 18 Nov 2024 09:00";

/// `$EDITOR` writes the recipient into the empty `To:` Postal leaves.
const EDITOR: &str = "perl -pi -e 's/^To: $/To: carol\\@example.com/'";

fn home(imap: &FakeImap, smtp: &FakeSmtp) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\nsignature = \"Zoë Example\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[smtp]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        imap.port,
        smtp.port,
        fake_oauth::shared().config()
    ))
}

fn postal(home: &Home) -> Terminal {
    home.terminal()
        .size(100, 30)
        .env("EDITOR", EDITOR)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

/// Ada's Message to the Account and Bob, the reply to an earlier one.
fn from_ada(subject: &str, body: FakeBody) -> FakeMessage {
    FakeMessage {
        from_address: "ada@example.com",
        ..FakeMessage::new("Ada Lovelace", subject, DATE)
    }
    .to("Me", EMAIL)
    .cc("Bob", "bob@example.com")
    .header("To", &format!("Me <{EMAIL}>"))
    .header("Cc", "Bob <bob@example.com>")
    .header("Message-ID", "<notes-2@example.com>")
    .header("In-Reply-To", "<notes-1@example.com>")
    .header("References", "<notes-1@example.com>")
    .body(body)
}

/// The one Draft's path in the data directory.
fn draft_path(home: &Home) -> PathBuf {
    let drafts: Vec<PathBuf> = fs::read_dir(home.drafts_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "eml"))
        .collect();
    assert_eq!(drafts.len(), 1);
    drafts.into_iter().next().unwrap()
}

/// The one Draft's file, and where its forwarded attachments are.
fn draft(home: &Home) -> (String, PathBuf) {
    let path = draft_path(home);
    let stem = path.file_stem().unwrap();
    let forwarded = home.xdg_data_home().join("postal/forwarded").join(stem);
    (fs::read_to_string(&path).unwrap(), forwarded)
}

/// `F` on the Message shown once `shown` holds, the Draft as the editor
/// saved it, and what the server received once its preview sends it.
fn forwarded(home: &Home, smtp: &FakeSmtp, shown: &str) -> ((String, PathBuf), Received) {
    let mut t = postal(home);
    t.wait_until(|s| s.contains(shown)).unwrap();
    t.send(Key::Char('F')).unwrap();
    t.wait_until(saved).unwrap();
    let file = draft(home);
    preview_newest(&mut t);
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("Sent") && !s.contains(" Draft "))
        .unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    let received = smtp.received();
    assert_eq!(received.len(), 1);
    (file, received.into_iter().next().unwrap())
}

/// The header block the forwarded Message's text sits under.
fn forwarded_header(subject: &str) -> String {
    format!(
        "---------- Forwarded message ---------\nFrom: Ada Lovelace <ada@example.com>\n\
         Date: {SHOWN_DATE}\nSubject: {subject}\nTo: Me <{EMAIL}>\nCc: Bob <bob@example.com>\n"
    )
}

/// The sent Message's headers a forward sets, and its text, CRLF as LF.
fn sent(received: &Received) -> (Vec<String>, String) {
    let message = MessageParser::default()
        .parse(received.data.as_bytes())
        .unwrap();
    // A forward starts a Thread of its own.
    assert_eq!(message.in_reply_to().as_text(), None, "{}", received.data);
    assert_eq!(message.references().as_text(), None, "{}", received.data);
    let to: Vec<String> = message
        .to()
        .unwrap()
        .iter()
        .map(|a| a.address().unwrap().to_owned())
        .collect();
    assert_eq!(message.cc(), None);
    let text = message.body_text(0).unwrap().replace("\r\n", "\n");
    (to, text)
}

#[test]
fn a_forward_sends_the_text_message_below_its_header_to_whom_the_editor_names() {
    let imap = FakeImap::start(vec![from_ada(
        "Weekly notes",
        FakeBody::plain("Line one\r\n\r\n> quoted before\r\nLine two\r\n"),
    )]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let ((file, _), received) = forwarded(&home, &smtp, "Line two");

    // Forwarded, not quoted: its own `>` lines stay as they came.
    let text = format!(
        "\n\n-- \nZoë Example\n\n{}\nLine one\n\n> quoted before\nLine two\n",
        forwarded_header("Weekly notes")
    );
    assert_eq!(
        file,
        format!("To: carol@example.com\nCc: \nSubject: Fwd: Weekly notes\n{text}")
    );
    assert_eq!(received.rcpt_to, ["carol@example.com"]);
    let parsed = MessageParser::default()
        .parse(received.data.as_bytes())
        .unwrap();
    assert_eq!(parsed.subject(), Some("Fwd: Weekly notes"));
    assert_eq!(parsed.attachment_count(), 0);
    assert_eq!(
        sent(&received),
        (
            vec!["carol@example.com".to_owned()],
            text.strip_prefix('\n').unwrap().to_owned()
        )
    );
}

#[test]
fn a_forward_of_html_mail_sends_its_text_laid_out_unquoted() {
    let html = FakeBody::text(
        "html",
        "7bit",
        "<html><head><style>p { color: red }</style></head><body>\
         <p>Hello <a href=\"https://example.com/x\">there</a></p><p>Second &amp; last</p>\
         </body></html>",
    );
    let imap = FakeImap::start(vec![from_ada("Fwd: Plans", html)]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let ((file, _), received) = forwarded(&home, &smtp, "Second & last");

    // Forwarded once already: the subject gains no second `Fwd:`.
    let text = format!(
        "\n\n-- \nZoë Example\n\n{}\nHello there\n\nSecond & last\n",
        forwarded_header("Fwd: Plans")
    );
    assert_eq!(
        file,
        format!("To: carol@example.com\nCc: \nSubject: Fwd: Plans\n{text}")
    );
    assert_eq!(
        sent(&received),
        (
            vec!["carol@example.com".to_owned()],
            text.strip_prefix('\n').unwrap().to_owned()
        )
    );
}

/// Each attachment of `message`: its name, its type and its bytes decoded.
fn attachments(message: &str) -> Vec<(String, String, Vec<u8>)> {
    let parsed = MessageParser::default().parse(message.as_bytes()).unwrap();
    parsed
        .attachments()
        .map(|part| {
            let kind = part.content_type().unwrap();
            let kind = format!("{}/{}", kind.ctype(), kind.subtype().unwrap_or_default());
            let name = part.attachment_name().unwrap().to_owned();
            (name, kind, part.contents().to_vec())
        })
        .collect()
}

#[test]
fn a_forward_sends_the_attachments_of_the_message_with_it() {
    let body = FakeBody::multipart(
        "mixed",
        vec![
            FakeBody::plain("See the files."),
            FakeBody::file("application", "pdf").disposition_param("filename", "report.pdf"),
            FakeBody::file("application", "pdf").disposition_param("filename", "report.pdf"),
        ],
    );
    let imap = FakeImap::start(vec![from_ada("Contract", body)]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let ((file, dir), received) = forwarded(&home, &smtp, "See the files.");

    // Two attachments of one name both go out under it.
    let text = format!(
        "\n\n-- \nZoë Example\n\n{}\nSee the files.\n",
        forwarded_header("Contract")
    );
    assert_eq!(
        file,
        format!(
            "To: carol@example.com\nCc: \nSubject: Fwd: Contract\nAttach: {}\nAttach: {}\n{text}",
            dir.join("0/report.pdf").display(),
            dir.join("1/report.pdf").display(),
        )
    );
    assert_eq!(sent(&received).1, text.strip_prefix('\n').unwrap());
    let bytes = vec![0, 1, 2, 3, 4, 5];
    assert_eq!(
        attachments(&received.data),
        [
            (
                "report.pdf".to_owned(),
                "application/pdf".to_owned(),
                bytes.clone()
            ),
            ("report.pdf".to_owned(), "application/pdf".to_owned(), bytes),
        ]
    );

    // Sent, its files go the next time Postal opens.
    assert!(dir.exists());
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("See the files.")).unwrap();
    assert!(!dir.exists());
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

// The Draft must not open with an attachment still on its way.
#[test]
fn a_forward_opens_its_draft_once_the_attachments_are_in() {
    let body = FakeBody::multipart(
        "mixed",
        vec![
            FakeBody::plain("See the files."),
            FakeBody::file("application", "pdf").disposition_param("filename", "report.pdf"),
        ],
    );
    let imap = FakeImap::start(vec![from_ada("Contract", body)]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("See the files.")).unwrap();
    imap.stall(Stall::Part);
    t.send(Key::Char('F')).unwrap();
    t.wait_until(|s| s.contains("fetching the Message…"))
        .unwrap();
    assert!(!saved(&t.screen()));

    imap.release();
    t.wait_until(saved).unwrap();
    assert!(draft(&home).0.contains("report.pdf\n"));
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

// A Forward's in-flight request dies with the fetch connection and must be
// asked again once the connection is back.
#[test]
fn a_forward_whose_fetch_connection_drops_asks_again_after_reconnect() {
    let body = FakeBody::multipart(
        "mixed",
        vec![
            FakeBody::plain("See the files."),
            FakeBody::file("application", "pdf").disposition_param("filename", "report.pdf"),
        ],
    );
    let imap = FakeImap::start(vec![from_ada("Contract", body)]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("See the files.")).unwrap();
    imap.stall(Stall::Part);
    t.send(Key::Char('F')).unwrap();
    t.wait_until(|s| s.contains("fetching the Message…"))
        .unwrap();

    imap.fail(Outage::Offline);
    imap.release();
    t.wait_until(|s| s.contains("offline")).unwrap();
    assert!(!saved(&t.screen()));

    imap.recover();
    t.wait_until(saved).unwrap();
    assert!(draft(&home).0.contains("report.pdf\n"));
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}
