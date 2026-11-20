use std::slice::SliceIndex;

use crate::{attribution, html_text};

/// The rule Outlook draws above the header of the mail it quotes, which
/// then runs to the end with no element around it. Lowercase.
const OUTLOOK_RULES: [&str; 2] = ["border-top:solid #e1e1e1", "border-top:solid #b5c4df"];

/// The bold labels that make a block the header of a quoted mail.
const LABELS: usize = 3;

struct Tag<'a> {
    start: usize,
    end: usize,
    closing: bool,
    name: &'a str,
}

impl Tag<'_> {
    fn is(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
    }
}

/// HTML up to the Quote it ends in: each `<blockquote>` with no words
/// after it and the line saying whom it quotes, or Outlook's header and
/// all under it. `None` when it ends in none, or says nothing else (ADR 0015).
pub fn cut(html: &str) -> Option<&str> {
    let tags = tags(html);
    let outlook = tags.iter().find(|tag| {
        let tag = part(html, tag.start..tag.end).to_ascii_lowercase();
        OUTLOOK_RULES.iter().any(|rule| tag.contains(rule))
    });

    let header = header(html, &tags);
    let rule = outlook.map(|tag| tag.start);
    let mut at = rule.into_iter().chain(header).min().unwrap_or(html.len());
    while let Some(quote) = last_quote(html, &tags, at) {
        at = quote;
    }

    (at < html.len() && !blank(part(html, ..at))).then(|| part(html, ..at))
}

/// Every tag of `html` in order, comments left out.
fn tags(html: &str) -> Vec<Tag<'_>> {
    let mut tags = Vec::new();
    let mut from = 0;
    while let Some(start) = part(html, from..).find('<').map(|at| from + at) {
        let rest = part(html, start..);
        if rest.starts_with("<!--") {
            from = rest.find("-->").map_or(html.len(), |end| start + end + 3);
            continue;
        }

        // A bare `<` in the text opens no tag.
        if !part(rest, 1..).starts_with(|c: char| c.is_ascii_alphabetic() || c == '/') {
            from = start + 1;
            continue;
        }

        let Some(end) = rest.find('>').map(|at| start + at + 1) else {
            break;
        };

        let inside = part(html, start + 1..end - 1);
        let closing = inside.starts_with('/');
        let name = inside
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default();
        tags.push(Tag {
            start,
            end,
            closing,
            name,
        });

        from = end;
    }

    tags
}

/// Where the header Outlook writes above the mail it quotes starts, with
/// no rule over it: the first block of three bold labels, `From:`, `Sent:`
/// and `To:` in any language, that holds an address.
fn header(html: &str, tags: &[Tag<'_>]) -> Option<usize> {
    let mut block = None;
    let mut labels = 0;
    for (index, tag) in tags.iter().enumerate() {
        if tag.closing {
            continue;
        }

        if tag.is("p") || tag.is("div") {
            (block, labels) = (Some(index), 0);
            continue;
        }

        if !(tag.is("b") || tag.is("strong")) {
            continue;
        }

        let Some(end) = closing(tags, index) else {
            continue;
        };

        labels += usize::from(
            html_text::strip(part(html, tag.end..end))
                .trim()
                .ends_with(':'),
        );
        if let Some(block) = block.filter(|_| labels >= LABELS) {
            let start = tags[block].start;
            let end = closing(tags, block).unwrap_or(html.len());
            if part(html, start..end).contains('@') {
                return Some(start);
            }
        }
    }

    None
}

/// Where the next closing tag named as `tags[opening]` starts.
fn closing(tags: &[Tag<'_>], opening: usize) -> Option<usize> {
    let name = tags[opening].name;
    let after = tags[opening + 1..].iter();
    after
        .filter(|tag| tag.closing)
        .find(|tag| tag.is(name))
        .map(|tag| tag.start)
}

/// Where the quote that ends `html` before `end` starts, the line naming whom it
/// quotes included: the last `<blockquote>` in no other, when no words
/// follow it.
fn last_quote(html: &str, tags: &[Tag<'_>], end: usize) -> Option<usize> {
    let mut depth = 0_usize;
    // The index of its opening tag, and where it closes.
    let mut last = None;
    for (index, tag) in tags.iter().enumerate() {
        if tag.start >= end {
            break;
        }

        if !tag.is("blockquote") {
            continue;
        }

        if tag.closing {
            if depth == 1
                && let Some((_, closed)) = &mut last
            {
                *closed = tag.end.min(end);
            }

            depth = depth.saturating_sub(1);
        } else {
            if depth == 0 {
                // Left open, it runs to the end.
                last = Some((index, end));
            }

            depth += 1;
        }
    }

    let (opening, closed) = last?;
    blank(part(html, closed..end)).then(|| attributed(html, tags, opening))
}

/// Where the line right above the quote opened by `tags[quote]` starts
/// when it names whom the quote is of; else where the quote does.
fn attributed(html: &str, tags: &[Tag<'_>], quote: usize) -> usize {
    let start = tags[quote].start;
    let mut line = String::new();
    let mut after = start;
    for tag in tags[..quote].iter().rev() {
        let text = html_text::strip(part(html, tag.end..after));
        if !text.trim().is_empty() {
            line.insert_str(0, &text);
        }

        after = tag.start;
        if !line.is_empty() && html_text::breaks(tag.name) {
            let opens = !tag.closing && !tag.is("br");
            let from = if opens { tag.start } else { tag.end };
            return if attribution::names(&line) {
                from
            } else {
                start
            };
        }
    }

    start
}

/// `range` of `html`, whose ends are always those of tags.
fn part<R: SliceIndex<str, Output = str>>(html: &str, range: R) -> &str {
    html.get(range).unwrap_or_default()
}

fn blank(html: &str) -> bool {
    html_text::strip(html).trim().is_empty()
}
