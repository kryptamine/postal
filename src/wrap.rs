use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// `text` in lines at most `width` columns wide, broken at spaces, and
/// inside a word too long for a line. A line quoted with `>` repeats its
/// prefix on each line it wraps to. Trailing blank lines are dropped.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let width = width.max(1);
    for line in text.lines() {
        let line = printable(line);
        let (prefix, rest) = quote_prefix(&line);
        if prefix.width() < width {
            wrap_line(prefix, rest, width, &mut lines);
        } else {
            wrap_line("", &line, width, &mut lines);
        }
    }

    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }

    lines
}

/// A tab as four spaces, and no other control character.
fn printable(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for c in line.chars() {
        match c {
            '\t' => out.push_str("    "),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }

    out
}

/// The run of `>` and spaces a quoted line starts with, and the rest.
fn quote_prefix(line: &str) -> (&str, &str) {
    if !line.starts_with('>') {
        return ("", line);
    }

    let rest = line.trim_start_matches(['>', ' ']);
    (&line[..line.len() - rest.len()], rest)
}

fn wrap_line(prefix: &str, text: &str, width: usize, lines: &mut Vec<String>) {
    let room = width - prefix.width();
    let mut line = prefix.to_owned();
    let mut used = 0;
    // `started`: the line holds a word, maybe an empty one from leading
    // spaces. `broken`: it follows a break, where spaces are dropped.
    let (mut started, mut broken) = (false, false);
    for word in text.split(' ') {
        let word_width = word.width();
        if started {
            if used + 1 + word_width <= room {
                line.push(' ');
                line.push_str(word);
                used += 1 + word_width;
                continue;
            }

            lines.push(line.trim_end().to_owned());
            prefix.clone_into(&mut line);
            (started, broken) = (false, true);
        }

        if broken && word.is_empty() {
            continue;
        }

        let mut word = word;
        while word.width() > room {
            let (head, tail) = split_at_width(word, room);
            line.push_str(head);
            lines.push(std::mem::replace(&mut line, prefix.to_owned()));
            word = tail;
        }

        line.push_str(word);
        used = word.width();
        started = true;
    }

    lines.push(line.trim_end().to_owned());
}

/// The longest head of `word` that fits `columns`, and the rest; the head
/// holds at least one grapheme, so a wide one on a narrow line still moves on.
fn split_at_width(word: &str, columns: usize) -> (&str, &str) {
    let mut used = 0;
    let mut end = 0;
    for (start, grapheme) in word.grapheme_indices(true) {
        used += grapheme.width();
        if used > columns && end > 0 {
            break;
        }

        end = start + grapheme.len();
    }

    word.split_at(end)
}
