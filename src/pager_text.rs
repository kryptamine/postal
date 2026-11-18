use std::io::{self, Write};

use ratatui::style::Style;

use crate::{ansi, preview::Preview, preview_pane::LABELS, theme::Theme};

/// `preview` as the bytes piped to the pager: the header block, a blank
/// line, then the body, in the Theme's ANSI styles.
pub fn ansi(preview: &Preview, theme: &Theme) -> Vec<u8> {
    let mut out = Vec::new();
    // Writing to a `Vec` cannot fail.
    let _ = write(&mut out, preview, theme);
    out
}

fn write(out: &mut Vec<u8>, preview: &Preview, theme: &Theme) -> io::Result<()> {
    let header = [
        (&preview.from, Style::new()),
        (&preview.subject, theme.title),
        (&preview.date, Style::new()),
    ];

    for (label, (value, style)) in LABELS.into_iter().zip(header) {
        ansi::styled(out, label, theme.muted)?;
        ansi::styled(out, value, style)?;
        out.write_all(b"\n")?;
    }

    out.write_all(b"\n")?;
    for line in &preview.body {
        for segment in line.segments() {
            ansi::styled(out, segment.text, theme.marked(segment.marks))?;
        }

        out.write_all(b"\n")?;
    }

    Ok(())
}
