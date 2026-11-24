use crate::domain::operator::Operator;

/// One piece of a Query between spaces, a quoted value keeping its spaces:
/// `-from:"Petr Novak"` is negated, of the operator `from`, of value `Petr Novak`.
pub struct Token<'a> {
    /// Where it starts, its `-` included.
    pub start: usize,
    pub negated: bool,
    pub head: Head<'a>,
    /// Where the value starts, its opening quote included.
    pub value_at: usize,
    /// Without its quotes.
    pub value: &'a str,
    pub quoted: bool,
    /// Quoted, with the closing quote still to type.
    pub open: bool,
}

/// What comes before a token's value.
#[derive(Clone, Copy)]
pub enum Head<'a> {
    /// No `name:`.
    Bare,
    Operator(Operator),
    /// A run of ASCII letters before a colon that names no operator.
    Unknown(&'a str),
}

/// The Query's tokens, left to right.
pub fn tokens(query: &str) -> impl Iterator<Item = Token<'_>> {
    let mut rest = query;
    std::iter::from_fn(move || {
        rest = rest.trim_start();
        if rest.is_empty() {
            return None;
        }

        let at = query.len() - rest.len();
        let (token, after) = token(rest, at);
        rest = after;
        Some(token)
    })
}

/// The token `text` starts with, `at` bytes into the Query, and the text
/// after it.
fn token(text: &str, at: usize) -> (Token<'_>, &str) {
    // A `-` alone is a word, not a negation of nothing.
    let unsigned = text
        .strip_prefix('-')
        .filter(|rest| rest.starts_with(|c: char| !c.is_whitespace()));
    let negated = unsigned.is_some();
    let text = unsigned.unwrap_or(text);

    let letters = text
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(text.len());
    let named = text
        .split_at_checked(letters)
        .and_then(|(name, rest)| Some((name, rest.strip_prefix(':')?)))
        .filter(|(name, _)| !name.is_empty());
    let (name, text) = named.map_or((None, text), |(name, rest)| (Some(name), rest));
    let head = name.map_or(Head::Bare, |name| {
        Operator::named(name).map_or(Head::Unknown(name), Head::Operator)
    });

    let value_at = at + usize::from(negated) + name.map_or(0, |name| name.len() + 1);
    let (value, quoted, open, rest) = text.strip_prefix('"').map_or_else(
        || {
            let end = text.find(char::is_whitespace).unwrap_or(text.len());
            let (value, rest) = text.split_at_checked(end).unwrap_or((text, ""));
            (value, false, false, rest)
        },
        |inner| match inner.split_once('"') {
            Some((value, rest)) => (value, true, false, rest),
            None => (inner, true, true, ""),
        },
    );

    let token = Token {
        start: at,
        negated,
        head,
        value_at,
        value,
        quoted,
        open,
    };
    (token, rest)
}
