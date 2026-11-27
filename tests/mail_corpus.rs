//! Every `.eml` in `tests/fixtures/` is a case: Postal serves it under its
//! own headers, and `|` hands it whole to the pager in the rendered and the
//! text View. Its `X-Postal-Case` header says what the snapshot protects.

use crate::support;

use mail_parser::MessageParser;
use support::{fake_imap::FakeMessage, pager::paged};
use termlens::Key;

/// `copy` without its escape sequences: CSI styling and OSC 8 hyperlinks.
fn plain(copy: &[u8]) -> String {
    let copy = String::from_utf8(copy.to_vec()).unwrap();
    let mut out = String::new();
    let mut chars = copy.chars();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }

        match chars.next() {
            Some('[') => _ = chars.find(|c| ('@'..='~').contains(c)),
            Some(']') => _ = chars.find(|&c| c == '\\' || c == '\x07'),
            _ => {}
        }
    }

    out.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

fn case(raw: &[u8]) -> String {
    let parsed = MessageParser::default().parse(raw).unwrap();
    let case = parsed
        .header_raw("X-Postal-Case")
        .expect("no X-Postal-Case header");
    case.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn every_fixture_mail_shows_as_its_snapshot() {
    insta::glob!("fixtures/*.eml", |path| {
        let raw = std::fs::read(path).unwrap();
        let rendered = plain(&paged(FakeMessage::eml(&raw), &[Key::Char('|')], true));
        let text = plain(&paged(
            FakeMessage::eml(&raw),
            &[Key::Char('v'), Key::Char('|')],
            true,
        ));
        insta::with_settings!({ description => case(&raw), omit_expression => true }, {
            insta::assert_snapshot!(format!("── rendered ──\n{rendered}\n\n── text ──\n{text}\n"));
        });
    });
}
