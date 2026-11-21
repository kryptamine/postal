use lettre::message::Mailbox;

use crate::correspondent::{self, Correspondent};

/// A Draft's text once each recipient written as words is expanded where
/// one Correspondent matches it.
pub struct Expanded {
    pub text: String,
    /// Each recipient several match, with them, best first.
    pub open: Vec<(String, Vec<Correspondent>)>,
    /// Each recipient none matches, left as written.
    pub unmatched: Vec<String>,
}

/// Whether `text` has a recipient to expand.
pub fn needed(text: &str) -> bool {
    let mut found = false;
    rewrite(text, |entry| {
        found |= !is_address(entry);
        None
    });
    found
}

/// Expands each recipient of `text` that is not an address against
/// `known`, ranked best first.
pub fn expand(text: &str, known: &[Correspondent]) -> Expanded {
    let (mut open, mut unmatched) = (Vec::new(), Vec::new());
    let text = rewrite(text, |entry| {
        if is_address(entry) {
            return None;
        }

        let query: Vec<String> = correspondent::words(entry).collect();
        let mut matches = known.iter().filter(|c| c.matches(&query));
        match (matches.next(), matches.next()) {
            (Some(only), None) => return Some(only.mailbox()),
            (Some(best), Some(next)) => {
                let mut all = vec![best.clone(), next.clone()];
                all.extend(matches.cloned());
                open.push((entry.to_owned(), all));
            }
            (None, _) => unmatched.push(entry.to_owned()),
        }

        None
    });

    Expanded {
        text,
        open,
        unmatched,
    }
}

/// `text` with the recipient written as `entry` replaced by `mailbox`.
pub fn choose(text: &str, entry: &str, mailbox: &str) -> String {
    let mut done = false;
    rewrite(text, |written| {
        let hit = !done && written == entry;
        done |= hit;
        hit.then(|| mailbox.to_owned())
    })
}

fn is_address(entry: &str) -> bool {
    entry.parse::<Mailbox>().is_ok()
}

/// `text` with each recipient of `To` and `Cc` that `replace` gives a new
/// value for replaced, those headers unfolded; as it was when its header
/// does not read.
fn rewrite(text: &str, mut replace: impl FnMut(&str) -> Option<String>) -> String {
    let text = text.replace("\r\n", "\n");
    let (head, body) = text.split_once("\n\n").unwrap_or((&text, ""));
    let Some(fields) = fields(head) else {
        return text;
    };

    let mut out = String::with_capacity(text.len());
    for (name, lines) in fields {
        let recipients = ["to", "cc"].contains(&name.trim().to_ascii_lowercase().as_str());
        if !recipients {
            for line in lines {
                out.push_str(line);
                out.push('\n');
            }

            continue;
        }

        let value = lines
            .iter()
            .enumerate()
            .map(|(n, line)| {
                if n == 0 {
                    line.split_once(':').map_or("", |(_, v)| v)
                } else {
                    line
                }
            })
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ");
        let entries: Vec<String> = entries(&value)
            .into_iter()
            .map(|entry| replace(entry).unwrap_or_else(|| entry.to_owned()))
            .collect();
        out.push_str(name);
        out.push_str(": ");
        out.push_str(&entries.join(", "));
        out.push('\n');
    }

    if text.contains("\n\n") {
        out.push('\n');
        out.push_str(body);
    }

    out
}

/// Each header's name and lines, a folded line kept with the one before;
/// `None` when a line is no header.
fn fields(head: &str) -> Option<Vec<(&str, Vec<&str>)>> {
    let mut fields: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in head.lines() {
        if line.starts_with([' ', '\t']) {
            fields.last_mut()?.1.push(line);
            continue;
        }

        let (name, _) = line.split_once(':')?;
        fields.push((name, vec![line]));
    }

    Some(fields)
}

/// The recipients of `value`, trimmed, split at each comma outside quotes
/// and angle brackets; empty ones left out.
fn entries(value: &str) -> Vec<&str> {
    let (mut entries, mut start) = (Vec::new(), 0);
    let (mut quoted, mut angled) = (false, false);
    for (at, c) in value.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '<' if !quoted => angled = true,
            '>' if !quoted => angled = false,
            ',' if !quoted && !angled => {
                entries.push(value.get(start..at).unwrap_or_default());
                start = at + 1;
            }
            _ => {}
        }
    }

    entries.push(value.get(start..).unwrap_or_default());
    entries
        .into_iter()
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .collect()
}
