use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// The longest prefix of `text` that fits `width` columns, cut on a grapheme
/// boundary, and whether it was cut. A cut prefix leaves one column for `…`.
pub fn fit(text: &str, width: usize) -> (&str, bool) {
    if text.width() <= width {
        return (text, false);
    }
    let budget = width.saturating_sub(1);
    let mut used = 0;
    let mut end = 0;
    for (start, grapheme) in text.grapheme_indices(true) {
        used += grapheme.width();
        if used > budget {
            break;
        }
        end = start + grapheme.len();
    }
    (&text[..end], true)
}
