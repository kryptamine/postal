//! A Message's attachments, from its `BODYSTRUCTURE`: a marker on its card,
//! and a line naming each under the header of the Preview, the Thread, the
//! pager and the editor's file; each fetched on demand, to open or save.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage, Outage},
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

/// Whether the Preview shows the body, which the Snippet in the list
/// comes before.
fn body_shown(screen: &Screen) -> bool {
    let rule = (0..screen.rows()).find(|&row| screen.row_text(row).contains('├'));
    rule.is_some_and(|row| screen.row_text(row + 1).contains("See the files."))
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
    assert!(!screen.contains("file-4"), "{screen}");
    t.send(Key::Char('a')).unwrap();
    t.wait_until(|s| s.contains("more (")).unwrap();
    t.send(Key::Esc).unwrap();
    t.wait_until(|s| !s.contains("more (")).unwrap();
    quit(t);
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
    t.wait_until(body_shown).unwrap();
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

fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out");
        thread::sleep(Duration::from_millis(20));
    }
}

fn cached_sizes(home: &Home) -> Vec<i64> {
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    let mut statement = cache.prepare("SELECT length(raw) FROM body").unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

// A large attachment would slow the Preview and push bodies out of the
// Cache's LRU: only the header and the text come, nested as sent.
#[test]
fn the_preview_fetches_the_text_and_never_the_attachments() {
    let alternative = FakeBody::multipart(
        "alternative",
        vec![
            FakeBody::plain("See the files."),
            FakeBody::text("html", "7bit", "<p>See the <b>files</b>.</p>"),
        ],
    );
    let body = FakeBody::multipart(
        "mixed",
        vec![
            alternative,
            FakeBody::file("application", "pdf")
                .lines(1000)
                .disposition_param("filename", "report.pdf"),
        ],
    );
    let server = FakeImap::start(vec![message("Ada", "Contract", 18, body)]);
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    t.wait_until(|s| shown(s, "report.pdf")).unwrap();
    wait_for(|| !cached_sizes(&home).is_empty());
    quit(t);

    let commands = server.commands();
    assert!(
        !commands
            .iter()
            .any(|c| c.contains("BODY.PEEK[]") || c.contains("BODY.PEEK[2]")),
        "{commands:#?}"
    );
    assert!(cached_sizes(&home)[0] < 1000, "{:?}", cached_sizes(&home));
}

/// The contract's photo as sent, `AAECAwQF`, decoded.
const PHOTO: [u8; 6] = [0, 1, 2, 3, 4, 5];

// Save writes the picked part decoded, and never over a file the download
// directory has: the taken name gets a number.
#[test]
fn save_writes_the_picked_attachment_beside_a_file_of_its_name() {
    let server = FakeImap::start(vec![message("Ada", "Contract", 18, contract())]);
    let home = Home::for_server(&server);
    let downloads = tempfile::tempdir().unwrap();
    fs::write(downloads.path().join("photo.jpg"), "older").unwrap();
    let mut t = home
        .terminal()
        .size(120, 30)
        .env("XDG_DOWNLOAD_DIR", downloads.path())
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    t.wait_until(|s| shown(s, "report.pdf")).unwrap();
    t.send(Key::Char('s')).unwrap();
    t.wait_until(|s| s.contains("a report.pdf 56 KB · s photo.jpg 6 B"))
        .unwrap();
    t.send(Key::Char('s')).unwrap();
    t.wait_until(|s| s.contains("saved to") && !s.contains("s photo.jpg"))
        .unwrap();
    quit(t);

    let saved = fs::read(downloads.path().join("photo (1).jpg")).unwrap();
    assert_eq!(saved, PHOTO);
    let older = fs::read_to_string(downloads.path().join("photo.jpg")).unwrap();
    assert_eq!(older, "older");
    let commands = server.commands();
    assert!(
        commands.iter().any(|c| c.ends_with("(UID BODY.PEEK[3])")),
        "{commands:#?}"
    );
}

// Open writes the part where the cache keeps opened ones and hands it to
// the desktop's opener, here a stub on `PATH`; what an earlier run opened
// is gone once the TUI starts.
#[test]
fn open_hands_the_attachment_at_the_cursor_to_the_opener() {
    let server = FakeImap::start(vec![message("Ada", "Contract", 18, contract())]);
    let home = Home::for_server(&server);
    let opened = home.xdg_cache_home().join("postal/attachments");
    fs::create_dir_all(&opened).unwrap();
    fs::write(opened.join("earlier.txt"), "earlier").unwrap();
    let stubs = tempfile::tempdir().unwrap();
    let record = stubs.path().join("handed");
    let script = format!("#!/bin/sh\nprintf '%s' \"$1\" > '{}'\n", record.display());
    for opener in ["open", "xdg-open"] {
        let path = stubs.path().join(opener);
        fs::write(&path, &script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let mut path = stubs.path().as_os_str().to_owned();
    path.push(":");
    path.push(std::env::var_os("PATH").unwrap());
    let mut t = home
        .terminal()
        .size(120, 30)
        .env("PATH", &path)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    t.wait_until(|s| shown(s, "report.pdf")).unwrap();
    assert!(!opened.join("earlier.txt").exists());
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| shown(s, "report.pdf") && s.contains(" 1/1 "))
        .unwrap();
    t.send(Key::Char('a')).unwrap();
    t.wait_until(|s| s.contains("a report.pdf")).unwrap();
    t.send(Key::Char('a')).unwrap();
    t.wait_until(|s| s.contains("opened report.pdf")).unwrap();
    wait_for(|| record.exists());
    quit(t);

    let handed = PathBuf::from(fs::read_to_string(&record).unwrap());
    assert_eq!(handed, opened.join("report.pdf"));
    // A thousand lines of `A`s, 57 zero octets each.
    assert_eq!(fs::read(&handed).unwrap(), vec![0; 57_000]);
}

// Offline, an attachment cannot come: it says so at once.
#[test]
fn an_attachment_picked_offline_says_so() {
    let server = FakeImap::start(vec![message("Ada", "Contract", 18, contract())]);
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    t.wait_until(|s| shown(s, "report.pdf")).unwrap();
    quit(t);

    server.fail(Outage::Offline);
    let mut t = postal(&home, 120);
    t.wait_until(|s| shown(s, "report.pdf")).unwrap();
    t.send(Key::Char('s')).unwrap();
    t.wait_until(|s| s.contains("a report.pdf")).unwrap();
    t.send(Key::Char('a')).unwrap();
    t.wait_until(|s| s.contains("offline · not downloaded"))
        .unwrap();
    quit(t);
}

// Mail names the file: one naming a path outside the download directory
// lands inside it all the same.
#[test]
fn a_saved_name_never_leaves_the_download_directory() {
    let body = mixed(vec![
        FakeBody::file("application", "pdf").disposition_param("filename", "../../evil.pdf"),
    ]);
    let server = FakeImap::start(vec![message("Ada", "Evil", 18, body)]);
    let home = Home::for_server(&server);
    let root = tempfile::tempdir().unwrap();
    let downloads = root.path().join("a/b");
    fs::create_dir_all(&downloads).unwrap();
    let mut t = home
        .terminal()
        .size(120, 30)
        .env("XDG_DOWNLOAD_DIR", &downloads)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    t.wait_until(|s| shown(s, "evil.pdf")).unwrap();
    t.send(Key::Char('s')).unwrap();
    t.wait_until(|s| s.contains("a ../../evil.pdf")).unwrap();
    t.send(Key::Char('a')).unwrap();
    t.wait_until(|s| s.contains("saved to")).unwrap();
    quit(t);

    let saved: Vec<_> = fs::read_dir(&downloads)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(saved, ["_.._evil.pdf"]);
    assert!(!root.path().join("evil.pdf").exists());
}
