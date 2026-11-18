use html2text::render::{TaggedLine, TaggedLineElement};

use super::decorator::{self, Annotation};
use crate::{paint::Paint, styled_line::StyledLine};

/// Characters newsletters pad preheaders and spacing with, which show as
/// nothing or as stray gaps.
const INVISIBLE: [char; 6] = [
    '\u{200b}', '\u{200c}', '\u{034f}', '\u{00ad}', '\u{feff}', '\u{2007}',
];

/// html2text's lines as Preview lines: invisible characters gone, trailing
/// spaces trimmed, blank lines at the ends dropped and runs of them made one.
pub fn styled(lines: Vec<TaggedLine<Vec<Annotation>>>) -> Vec<StyledLine> {
    let mut styled: Vec<StyledLine> = Vec::with_capacity(lines.len());
    let mut piece = String::new();
    for line in lines {
        let mut out = StyledLine::new();
        for element in line.iter() {
            let TaggedLineElement::Str(tagged) = element else {
                continue;
            };

            piece.clear();
            piece.extend(
                tagged
                    .s
                    .chars()
                    .filter(|c| !INVISIBLE.contains(c))
                    .map(|c| if c == '\u{a0}' { ' ' } else { c }),
            );
            let tag = &tagged.tag;
            out.push(
                &piece,
                decorator::marks(tag),
                Paint::default(),
                decorator::link(tag),
            );
        }

        out.trim_end();
        let after_blank = styled.last().is_none_or(|last| last.text.is_empty());
        if out.text.is_empty() && after_blank {
            continue;
        }

        styled.push(out);
    }

    while styled.last().is_some_and(|last| last.text.is_empty()) {
        styled.pop();
    }

    styled
}
