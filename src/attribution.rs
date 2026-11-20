/// A longer line above a Quote is the Message's own words.
const LONGEST: usize = 200;

/// Whether `line`, the text right above quoted mail, says whom it quotes, as
/// `On 4 Mar 2024, Bob <bob@example.com> wrote:` does in any language: it
/// ends in a colon and holds a date or an address.
pub fn names(line: &str) -> bool {
    let line = line.trim();
    line.ends_with(':')
        && line.chars().count() <= LONGEST
        && line.contains(|c: char| c.is_ascii_digit() || c == '@')
}
