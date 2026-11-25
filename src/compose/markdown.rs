use std::fmt::Write;

use comrak::{Options, markdown_to_html};

use crate::{compose::forward_draft::FORWARDED, render::quoted_text, render::signature::SEPARATOR};

/// The usual look for a quote, which a client without one would drop.
const QUOTE: &str = "margin:0 0 0 .8ex;border-left:1px solid #ccc;padding-left:1ex";
/// Text as written: its lines and spaces kept, in the mail's own font.
const WRITTEN: &str = "white-space:pre-wrap;font-family:inherit;margin:0";
const MONOSPACE: &str = "font-family:monospace";
const CELL: &str = "padding:2px 6px";
const HEADING_CELL: &str = "padding:2px 6px;border-bottom:1px solid #ccc";

/// The HTML a Draft's `body` is sent as beside its text: what the user
/// wrote as Markdown, then the signature and a trailing Quote or Forward
/// as written, the Quote's levels nested.
pub fn html(body: &str) -> String {
    let (head, tail) = split_tail(body);
    let (own, signature) = lines_at(head, SEPARATOR)
        .last()
        .map_or((head, ""), |at| head.split_at(at));
    let mut html = String::from("<!DOCTYPE html>\n<html><body>\n");
    html.push_str(&markdown(&unlazy(own)));
    html.push_str(&written(signature));
    html.push_str(&written(tail));
    html.push_str("</body></html>\n");
    html
}

/// `body` cut where a Forward's original or a Reply's Quote starts.
fn split_tail(body: &str) -> (&str, &str) {
    let forwarded = lines_at(body, FORWARDED).next();
    let at = forwarded.or_else(|| quoted_text::cut(body).map(str::len));
    at.map_or((body, ""), |at| body.split_at(at))
}

/// `text` with a blank line after each Quote line that a line of the
/// user's follows: `CommonMark` would take that line into the quote.
fn unlazy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut quoted = false;
    for line in text.split_inclusive('\n') {
        if quoted && !line.starts_with('>') && !line.trim().is_empty() {
            out.push('\n');
        }

        quoted = line.starts_with('>');
        out.push_str(line);
    }

    out
}

/// Where each line of `text` that is `line` starts, in bytes.
fn lines_at<'a>(text: &'a str, line: &'a str) -> impl Iterator<Item = usize> + 'a {
    let mut at = 0;
    text.split_inclusive('\n').filter_map(move |each| {
        let start = at;
        at += each.len();
        (each.strip_suffix('\n').unwrap_or(each) == line).then_some(start)
    })
}

/// `CommonMark` with GFM tables, strikethrough and autolinks; a line break
/// is a line break, and raw HTML is escaped.
fn markdown(text: &str) -> String {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.render.hardbreaks = true;
    options.render.escape = true;
    // With raw HTML escaped, every tag left is comrak's own to style.
    markdown_to_html(text, &options)
        .replace("<blockquote>", &format!("<blockquote style=\"{QUOTE}\">"))
        .replace("<pre>", &format!("<pre style=\"{MONOSPACE}\">"))
        .replace("<code", &format!("<code style=\"{MONOSPACE}\""))
        .replace("<table>", "<table style=\"border-collapse:collapse\">")
        // A cell aligned has attributes; one inserted here must not match.
        .replace("<th ", &format!("<th style=\"{HEADING_CELL}\" "))
        .replace("<th>", &format!("<th style=\"{HEADING_CELL}\">"))
        .replace("<td ", &format!("<td style=\"{CELL}\" "))
        .replace("<td>", &format!("<td style=\"{CELL}\">"))
}

/// `text` as written, each run of lines at one Quote level in a block of
/// its own inside as many quotes.
fn written(text: &str) -> String {
    let mut html = String::new();
    let mut runs: Vec<(usize, Vec<&str>)> = Vec::new();
    for line in text.lines() {
        let (level, line) = level(line);
        match runs.last_mut() {
            Some((last, lines)) if *last == level => lines.push(line),
            _ => runs.push((level, vec![line])),
        }
    }

    let mut open = 0;
    for (level, lines) in runs {
        while open > level {
            html.push_str("</blockquote>\n");
            open -= 1;
        }

        while open < level {
            // Writing to a String cannot fail.
            writeln!(html, "<blockquote style=\"{QUOTE}\">").ok();
            open += 1;
        }

        let text = lines.join("\n");
        let text = text.trim_matches('\n');
        if text.trim().is_empty() {
            continue;
        }

        write!(html, "<pre style=\"{WRITTEN}\">").ok();
        comrak::html::escape(&mut html, text).ok();
        html.push_str("</pre>\n");
    }

    html.push_str(&"</blockquote>\n".repeat(open));
    html
}

/// How deep in a Quote `line` is, `>` and `> >` alike, and its text.
fn level(mut line: &str) -> (usize, &str) {
    let mut level = 0;
    while let Some(rest) = line.strip_prefix('>') {
        level += 1;
        line = rest.strip_prefix(' ').unwrap_or(rest);
    }

    (level, line)
}
