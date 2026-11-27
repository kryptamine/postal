//! `compose-format = "markdown"` sends a Draft's body twice in a
//! `multipart/alternative`: as written in `text/plain`, and as HTML, its
//! Markdown converted and its signature, Quote and Forward kept as written.

use crate::support;

use std::fs;

use mail_parser::{MessageParser, MimeHeaders, PartType};
use support::{
    editor::{preview_newest, sending},
    fake_imap::{EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
};
use tempfile::TempDir;
use termlens::{Key, Screen, Terminal};

/// `$HOME`, with `$EDITOR` copying the Draft as written over the one
/// Postal opens.
struct Desk {
    home: TempDir,
}

impl Desk {
    fn new(draft: &str) -> Self {
        let home = tempfile::tempdir().unwrap();
        fs::write(home.path().join("next.eml"), draft).unwrap();
        fs::write(home.path().join("notes.txt"), "*not* Markdown\n").unwrap();
        Self { home }
    }

    /// Notes the format the editor is told, for the plugin to render it.
    fn editor(&self) -> String {
        let path = self.home.path();
        format!(
            "printenv POSTAL_COMPOSE_FORMAT > '{}'; cp '{}'",
            path.join("format").display(),
            path.join("next.eml").display()
        )
    }

    fn told(&self) -> String {
        fs::read_to_string(self.home.path().join("format")).unwrap()
    }
}

fn home(imap: &FakeImap, smtp: &FakeSmtp, format: &str) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\n{format}\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[smtp]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        imap.port,
        smtp.port,
        fake_oauth::shared().config()
    ))
}

fn listed(screen: &Screen) -> bool {
    screen.contains("Weekly notes")
        && !screen.contains(" Draft ")
        && !screen.contains(" Local Drafts ")
}

/// Postal over `imap` and `smtp`, its editor `editor` at home in `desk`.
fn postal(home: &Home, desk: &Desk, editor: &str) -> Terminal {
    home.terminal()
        .size(100, 30)
        .env("HOME", desk.home.path())
        .env("EDITOR", editor)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn inbox() -> FakeImap {
    FakeImap::start(vec![FakeMessage::new(
        "Ada Lovelace",
        "Weekly notes",
        "2024-11-18T09:00:00Z",
    )])
}

/// What the server took once `draft` was composed with `c` and sent from
/// the editor, and the format its editor was told.
fn sent(format: &str, draft: &str) -> (String, String) {
    let (imap, smtp, desk) = (inbox(), FakeSmtp::start(), Desk::new(draft));
    let home = home(&imap, &smtp, format);
    let mut t = postal(&home, &desk, &sending(&desk.editor()));
    t.wait_until(listed).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains("Sent") && listed(s)).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let received = smtp.received();
    assert_eq!(received.len(), 1);
    (received[0].data.clone(), desk.told())
}

/// The decoded `text/plain` and `text/html` parts of `data`.
fn parts(data: &str) -> (String, String) {
    let parsed = MessageParser::default().parse(data.as_bytes()).unwrap();
    let text = parsed.text_part(0).unwrap();
    assert_eq!(text.content_type().unwrap().subtype(), Some("plain"));
    let html = parsed.html_part(0).unwrap();
    let PartType::Html(html) = &html.body else {
        panic!("no text/html part: {data}");
    };

    let text = text.text_contents().unwrap().replace("\r\n", "\n");
    (text, html.replace("\r\n", "\n"))
}

const MARKDOWN: &str = "compose-format = \"markdown\"\n";

const WRITTEN: &str = "\
Hi Ada,
the plan, line by line:

- a **list** with [a link](https://example.com/plan)
- ~~struck~~ and https://example.com/bare

```
let x = 1;
```

| name | due |
| ---- | --: |
| Ada  | Mon |

<script>alert(1)</script>
";

#[test]
fn markdown_sends_the_body_as_written_beside_its_html() {
    let (data, told) = sent(
        MARKDOWN,
        &format!("To: bob@example.com\nSubject: Plan\n\n{WRITTEN}"),
    );
    assert_eq!(told, "markdown\n");
    assert!(data.contains("multipart/alternative"), "{data}");
    let (text, html) = parts(&data);
    assert_eq!(text, WRITTEN);
    // Raw HTML in the source is text in the HTML, never a tag.
    assert!(!html.contains("<script>"), "{html}");
    insta::assert_snapshot!(html);
}

#[test]
fn a_reply_keeps_its_signature_and_nests_its_quote_as_written() {
    let body = "Sounds good.\n\n-- \nBob\n----\n\nOn 18 Nov 2024, Ada Lovelace wrote:\n\
                > *Shall we?*\n> > Monday\n> back\n";
    let (data, _) = sent(
        MARKDOWN,
        &format!("To: ada@example.com\nSubject: Re: Plan\n\n{body}"),
    );
    let (text, html) = parts(&data);
    assert_eq!(text, body);
    // Under text, a line of dashes would make a setext heading of it.
    assert!(!html.contains("<h2"), "{html}");
    insta::assert_snapshot!(html);
}

#[test]
fn a_forward_converts_only_the_text_written_and_keeps_its_files() {
    let body = "**Read this.**\n\n---------- Forwarded message ---------\nFrom: Ada <ada@example.com>\n\
                Subject: Notes\n\n*as sent*, 1. not a list\n";
    let (data, _) = sent(
        MARKDOWN,
        &format!("To: bob@example.com\nSubject: Fwd: Notes\nAttach: notes.txt\n\n{body}"),
    );
    let parsed = MessageParser::default().parse(data.as_bytes()).unwrap();
    let kind = parsed.content_type().unwrap();
    assert_eq!(kind.subtype(), Some("mixed"), "{data}");
    let attachment = parsed.attachment(0).unwrap();
    assert_eq!(attachment.attachment_name(), Some("notes.txt"));
    assert_eq!(attachment.contents(), b"*not* Markdown\n");
    let (text, html) = parts(&data);
    assert_eq!(text, body);
    assert!(html.contains("<strong>Read this.</strong>"), "{html}");
    assert!(html.contains("*as sent*, 1. not a list"), "{html}");
}

#[test]
fn plain_sends_one_text_part_as_before() {
    let body = "- **not** converted\n";
    let (data, told) = sent(
        "",
        &format!("To: bob@example.com\nSubject: Plain\n\n{body}"),
    );
    assert_eq!(told, "plain\n");
    assert!(!data.contains("multipart"), "{data}");
    assert!(!data.contains("text/html"), "{data}");
    let parsed = MessageParser::default().parse(data.as_bytes()).unwrap();
    assert_eq!(parsed.body_text(0).unwrap().replace("\r\n", "\n"), body);
}

// The preview shows the Draft as it will be read: its HTML laid out, the
// Markdown gone; `v` brings the text that goes beside it back.
#[test]
fn a_markdown_drafts_preview_shows_its_html_and_v_its_text() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let desk = Desk::new("To: bob@example.com\nSubject: Plan\n\n# Plan\n\n- **first**\n- second\n");
    let home = home(&imap, &smtp, MARKDOWN);
    let mut t = postal(&home, &desk, &desk.editor());
    t.wait_until(listed).unwrap();
    t.send(Key::Char('c')).unwrap();
    preview_newest(&mut t);
    // Past the Toast the editor's exit left, which would date the snapshot.
    let rendered = t
        .snapshot_after(|s| s.contains("first") && !s.contains("Saved to Drafts"))
        .unwrap();
    assert!(!rendered.contains("**first**"), "{rendered}");
    insta::assert_snapshot!(rendered);
    t.send(Key::Char('v')).unwrap();
    t.wait_until(|s| s.contains("- **first**")).unwrap();
    t.send(Key::Char('v')).unwrap();
    t.wait_until(|s| s.contains("first") && !s.contains("**first**"))
        .unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

// An answer written right under a line it quotes is the user's own, not
// part of the quote, as CommonMark's lazy lines would make it.
#[test]
fn an_answer_under_a_quoted_line_stays_out_of_the_quote() {
    let body = "> Monday?\nYes, Monday.\n";
    let (data, _) = sent(
        MARKDOWN,
        &format!("To: ada@example.com\nSubject: Re: Plan\n\n{body}"),
    );
    let (text, html) = parts(&data);
    assert_eq!(text, body);
    assert!(
        html.contains("</blockquote>\n<p>Yes, Monday.</p>"),
        "{html}"
    );
}
