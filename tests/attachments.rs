//! A Message's attachments, from its `BODYSTRUCTURE`: a marker on its card,
//! and a line naming each under the header of the Preview, the Thread, the
//! pager and the editor's file.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{fs, path::Path};

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

const MARKER: &str = "+";

fn postal(home: &Home, cols: u16) -> Terminal {
    home.terminal()
        .size(cols, 30)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn message(sender: &'static str, subject: &str, day: u32, body: FakeBody) -> FakeMessage {
    FakeMessage::new(sender, subject, &format!("2024-11-{day:02}T09:00:00Z")).body(body)
}

fn mixed(parts: Vec<FakeBody>) -> FakeBody {
    let mut all = vec![FakeBody::plain("See the files.")];
    all.extend(parts);
    FakeBody::multipart("mixed", all)
}

/// A PDF of 57,000 octets named by its disposition, and a photo of 6
/// named only by its `Content-Type`.
fn contract() -> FakeBody {
    mixed(vec![
        FakeBody::file("application", "pdf")
            .lines(1000)
            .disposition_param("filename", "report.pdf"),
        FakeBody::file("image", "jpeg")
            .undisposed()
            .type_param("name", "photo.jpg"),
    ])
}

/// The marker left of the subject on the card of `sender`; `None` when no
/// card names it.
fn marker(screen: &Screen, sender: &str) -> Option<String> {
    let row = (0..screen.rows()).find(|&row| {
        let text = screen.row_text(row);
        text.get(..text.len().min(40))
            .is_some_and(|left| left.contains(sender))
    })?;
    Some(screen.cell(row + 1, 2)?.contents().trim().to_owned())
}

fn shown(screen: &Screen, text: &str) -> bool {
    screen.contains(text) && screen.contains("See the files.")
}

#[test]
fn a_message_with_attachments_is_marked_and_lists_them_under_the_header() {
    let server = FakeImap::start(vec![
        message("Bob", "Lunch", 17, FakeBody::plain("Noon?")),
        message("Ada", "Contract", 18, contract()),
    ]);
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    let listed = "report.pdf 56 KB · photo.jpg 6 B";
    let screen = t.snapshot_after(|s| shown(s, listed)).unwrap();
    assert_eq!(marker(&screen, "Ada").as_deref(), Some(MARKER), "{screen}");
    assert_eq!(marker(&screen, "Bob").as_deref(), Some(""), "{screen}");

    t.send(Key::Enter).unwrap();
    t.wait_until(|s| shown(s, listed) && s.contains(" 1/1 "))
        .unwrap();
    quit(t);
}

// Names outside ASCII come as RFC 2231 parameters, whole or in numbered
// segments, or as RFC 2047 encoded words; the RFC 2231 one wins.
#[test]
fn an_encoded_file_name_is_shown_decoded() {
    let names = mixed(vec![
        FakeBody::file("application", "pdf")
            .disposition_param("filename", "otchet.pdf")
            .disposition_param("filename*", "utf-8''%D0%BE%D1%82%D1%87%D1%91%D1%82.pdf"),
        FakeBody::file("text", "csv")
            .disposition_param("filename*0*", "utf-8''%E2%82%AC%20rates")
            .disposition_param("filename*1", ".csv"),
        FakeBody::file("image", "png")
            .disposition_param("filename", "=?UTF-8?B?0L/Qu9Cw0L0ucG5n?="),
    ]);
    let server = FakeImap::start(vec![message("Ada", "Names", 18, names)]);
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    t.wait_until(|s| shown(s, "отчёт.pdf 6 B · € rates.csv 6 B · план.png 6 B"))
        .unwrap();
    quit(t);
}

// An image the HTML shows by `cid:` is part of the text, not a file sent
// with it, though it has a file name.
#[test]
fn an_inline_image_is_not_listed() {
    let html = "<p>Our logo:</p><img src=\"cid:logo\">";
    let related = FakeBody::multipart(
        "related",
        vec![
            FakeBody::text("html", "7bit", html),
            FakeBody::file("image", "png")
                .disposition_param("filename", "logo.png")
                .inline("logo"),
        ],
    );
    let server = FakeImap::start(vec![message("Ada", "Logo", 18, related)]);
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    let screen = t.snapshot_after(|s| s.contains("Our logo")).unwrap();
    quit(t);
    assert!(!screen.contains("logo.png"), "{screen}");
    assert_eq!(marker(&screen, "Ada").as_deref(), Some(""), "{screen}");
}

// What does not fit the line is counted, never cut mid-name.
#[test]
fn a_line_too_narrow_for_every_attachment_counts_the_rest() {
    let names = [
        "file-1.pdf",
        "file-2.pdf",
        "file-3.pdf",
        "file-4.pdf",
        "file-5.pdf",
        "file-6.pdf",
    ];
    let files = names
        .iter()
        .map(|name| FakeBody::file("application", "pdf").disposition_param("filename", name))
        .collect();
    let server = FakeImap::start(vec![message("Ada", "Files", 18, mixed(files))]);
    let home = Home::for_server(&server);
    let mut t = postal(&home, 80);
    let listed = "file-1.pdf 6 B · file-2.pdf 6 B · file-3.pdf 6 B · +3 more";
    let screen = t.snapshot_after(|s| shown(s, listed)).unwrap();
    quit(t);
    assert!(!screen.contains("file-4"), "{screen}");
}

/// What `$PAGER` read after `|`, then what `$EDITOR` was handed after `E`,
/// of the contract.
fn handed_out() -> (String, String) {
    let server = FakeImap::start(vec![message("Ada", "Contract", 18, contract())]);
    let home = Home::for_server(&server);
    let dir = tempfile::tempdir().unwrap();
    let (paged_file, edited_file) = (dir.path().join("paged"), dir.path().join("edited"));
    // Each holds the screen until the test answers on the terminal.
    let hold = "printf done; read answer < /dev/tty";
    let pager = format!("cat > '{}'; {hold}", paged_file.display());
    let editor = format!("sh -c 'cp \"$1\" {}; {hold}' sh", edited_file.display());
    let mut t = home
        .terminal()
        .size(120, 30)
        .env("NO_COLOR", "1")
        .env("PAGER", Path::new(&pager))
        .env("EDITOR", Path::new(&editor))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    for key in ['|', 'E'] {
        t.wait_until(|s| s.alternate_screen() && shown(s, "report.pdf"))
            .unwrap();
        t.send(Key::Char(key)).unwrap();
        t.wait_until(|s| !s.alternate_screen() && s.contains("done"))
            .unwrap();
        t.send(Key::Enter).unwrap();
    }

    t.wait_until(|s| s.alternate_screen() && shown(s, "report.pdf"))
        .unwrap();
    quit(t);
    (
        fs::read_to_string(paged_file).unwrap(),
        fs::read_to_string(edited_file).unwrap(),
    )
}

// The pager and the editor show what the Preview's header does, so a
// Message read there is not taken for one with nothing attached.
#[test]
fn the_pager_and_the_editor_list_the_attachments_after_the_header() {
    let (paged, edited) = handed_out();
    let paged = paged.replace("\x1b[1m", "").replace("\x1b[0m", "");
    let listed = "report.pdf 56 KB · photo.jpg 6 B";
    let header = format!("Subject Contract\n{MARKER}       {listed}\n\n");
    assert!(paged.contains(&header), "{paged}");
    let header = format!("Subject: Contract\nAttachments: {listed}\n\n");
    assert!(edited.contains(&header), "{edited}");
}
