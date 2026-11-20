use crate::attribution;

/// The narrowest a mail client wraps plain text.
const WRAP: usize = 72;

/// Plain text up to the Quote it ends in: the `>` lines and the line
/// saying whom they quote. `None` when it ends in none, or says nothing
/// else.
pub fn cut(text: &str) -> Option<&str> {
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut quoted = false;
    while let Some(line) = lines.last()
        && (line.starts_with('>') || line.trim().is_empty())
    {
        quoted |= line.starts_with('>');
        lines.pop();
    }

    if !quoted {
        return None;
    }

    let named = match lines.as_slice() {
        [.., above, last]
            if wrapped(above, last) && attribution::names(&[*above, *last].concat()) =>
        {
            2
        }
        [.., last] if attribution::names(last) => 1,
        _ => 0,
    };

    lines.truncate(lines.len() - named);
    let kept: usize = lines.iter().map(|line| line.len()).sum();
    let (own, _) = text.split_at_checked(kept)?;
    (!own.trim().is_empty()).then_some(own)
}

/// Whether `line` was broken off the end of `above` for want of room: its
/// first word would not have fit there.
fn wrapped(above: &str, line: &str) -> bool {
    let word = line.split_whitespace().next().unwrap_or_default();
    let above = above.trim_end();
    !above.is_empty() && above.chars().count() + 1 + word.chars().count() > WRAP
}
