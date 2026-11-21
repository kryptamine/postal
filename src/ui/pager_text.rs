use std::io::{self, Write};

use ratatui::style::Style;

use crate::{
    account::browser, domain::message::Message, render::preview::Preview, ui::ansi,
    ui::message_header, ui::theme::Theme,
};

/// `preview` of `message` as the bytes piped to the pager: the header
/// block, a blank line, then the body, in the Theme's ANSI styles, each link an OSC 8
/// hyperlink a terminal can open on click.
pub fn ansi(message: &Message, preview: &Preview, theme: &Theme) -> Vec<u8> {
    let mut out = Vec::new();
    // Writing to a `Vec` cannot fail.
    write(&mut out, message, preview, theme).ok();
    out
}

fn write(out: &mut Vec<u8>, message: &Message, preview: &Preview, theme: &Theme) -> io::Result<()> {
    let [from, rest @ ..] = message_header::fields(theme, message);
    field(out, from, theme)?;
    out.write_all(b"  ")?;
    ansi::styled(
        out,
        message_header::date(&message.received).as_str(),
        theme.muted,
    )?;
    for row in rest {
        out.write_all(b"\n")?;
        field(out, row, theme)?;
    }

    out.write_all(b"\n")?;

    out.write_all(b"\n")?;
    for line in &preview.body {
        for segment in line.segments() {
            let style = theme.painted(segment.marks, segment.paint);
            match segment.link.filter(|url| browser::safe(url)) {
                Some(url) => ansi::hyperlink(out, url, segment.text, style)?,
                None => ansi::styled(out, segment.text, style)?,
            }
        }

        out.write_all(b"\n")?;
    }

    Ok(())
}

fn field(
    out: &mut Vec<u8>,
    (label, value, style): (&str, &str, Style),
    theme: &Theme,
) -> io::Result<()> {
    ansi::styled(out, label, theme.field)?;
    ansi::styled(out, value, style)
}
