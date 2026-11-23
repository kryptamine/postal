use clap::builder::{StyledStr, styling::AnsiColor};

const LOCKUP: &str = include_str!("../../assets/brand/postal-lockup.txt");
// The columns the stamp fills on each line, as `scripts/brand.py` draws it.
const STAMP: usize = 19;

/// The stamp in the terminal's own red; clap drops the colour when stdout
/// is no terminal.
pub fn lockup() -> StyledStr {
    let red = AnsiColor::Red.on_default();
    LOCKUP
        .lines()
        .map(|line| {
            let split = line
                .char_indices()
                .nth(STAMP)
                .map_or(line.len(), |(at, _)| at);
            let (stamp, word) = line.split_at(split);
            format!("{red}{stamp}{red:#}{word}")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .into()
}
