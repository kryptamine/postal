//! The layout process prints a grid as text when asked, which
//! `scripts/rendered-diff.py` diffs between two builds.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "a test fails by panicking"
)]

use std::{
    io::Write,
    process::{Command, Stdio},
};

fn laid_out_text(html: &str, cols: u16) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_postal"))
        .args(["__lay-out", "text", &cols.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(html.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn rows_and_hrefs_print_as_text() {
    let html = r#"<p>Read <a href="https://example.com/notes">the notes</a> 日本</p>"#;
    let text = laid_out_text(html, 30);
    assert_eq!(text.lines().next(), Some("fits"));
    assert!(
        text.lines()
            .any(|line| line.trim() == "Read the notes 日本"),
        "{text}"
    );
    assert!(text.ends_with("[1] https://example.com/notes\n"), "{text}");
    assert!(text.lines().all(|line| line == line.trim_end()), "{text}");
}
