use crate::{marks::Marks, styled_line::StyledLine};

/// The footer starts no higher than this share of the mail from its end,
/// or this many rows where the mail is short.
const SHARE: usize = 3;
const ROWS: usize = 8;
/// What a row of the footer says, lowercase, or links to.
const WORDS: [&str; 4] = ["©", "unsubscribe", "all rights reserved", "copyright"];
const ADDRESSES: [&str; 4] = ["unsub", "optout", "opt-out", "opt_out"];

/// Draws the footer muted, in the terminal's own colours: the block near
/// the end of the mail that holds its unsubscribe link or its copyright
/// line, and everything under it. Its links stay links.
pub fn mute(lines: &mut [StyledLine]) {
    let tail = lines.len() - (lines.len() / SHARE).max(ROWS).min(lines.len());
    let Some(marker) = (tail..lines.len()).find(|&row| marks_footer(&lines[row])) else {
        return;
    };

    let above = lines[..marker].iter().rposition(StyledLine::spacer);
    let block = above.map_or(0, |row| row + 1);
    // A block that starts higher up is the mail's body with the line in it.
    let start = if block < tail { marker } else { block };
    for line in &mut lines[start..] {
        line.restyle(Marks::MUTED);
    }
}

fn marks_footer(line: &StyledLine) -> bool {
    let text = line.text.to_lowercase();
    WORDS.iter().any(|word| text.contains(word))
        || line.segments().any(|segment| {
            segment.link.is_some_and(|url| {
                let url = url.to_lowercase();
                ADDRESSES.iter().any(|address| url.contains(address))
            })
        })
}
