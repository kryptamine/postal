use std::ops::Range;

use chrono::{DateTime, Days, Local, Months, NaiveDate};

use crate::{
    domain::folder::{Folder, Role},
    domain::operator::Operator,
    domain::query_token::{Head, Token, tokens},
};

/// A Query cut into terms, words and operators alike: see
/// docs/adr/0031-the-query-takes-operators.md.
pub struct Query {
    pub terms: Vec<Term>,
}

pub struct Term {
    pub negated: bool,
    pub kind: Kind,
    /// The `-` and `name:` of an operator, or the `-` of negated words:
    /// what the Query line highlights. Empty for plain words.
    pub head: Range<usize>,
}

pub enum Kind {
    /// Each word the start of a word in `field`; a phrase keeps them
    /// adjacent and in order.
    Words {
        field: Field,
        words: Vec<String>,
        phrase: bool,
    },
    /// Unread, or with `false`, read.
    Unread(bool),
    Attachment,
    /// A Folder, by its path or its role's label.
    Folder(String),
    /// Part of an Account's address.
    Account(String),
    /// Arrived before this second.
    Before(i64),
    /// Arrived at this second or after.
    After(i64),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// What a word without an operator matches: the sender, the subject,
    /// the Snippet and, of mail the Account sent, its recipients.
    Any,
    /// The From line, name and address.
    From,
    /// `To` and `Cc`: the Cache keeps them as one line.
    To,
    Subject,
}

impl Query {
    /// A term of an unknown operator is searched as words; one of a known
    /// operator whose value does not parse, as `from:` alone, is none.
    pub fn parse(text: &str, now: DateTime<Local>) -> Self {
        let terms = tokens(text).filter_map(|token| term(&token, now)).collect();
        Self { terms }
    }

    /// The words without an operator, but those negated: what nucleo ranks by.
    pub fn ranked(&self) -> String {
        let mut ranked = String::new();
        for term in self.terms.iter().filter(|t| !t.negated) {
            if let Kind::Words {
                field: Field::Any,
                words,
                ..
            } = &term.kind
            {
                for word in words {
                    if !ranked.is_empty() {
                        ranked.push(' ');
                    }

                    ranked.push_str(word);
                }
            }
        }

        ranked
    }

    /// Whether a Search reads `folder` of the Account at `email`: Trash and
    /// Spam only when an `in:` names them.
    pub fn reads<A>(&self, email: &str, folder: &Folder<A>) -> bool {
        let shunned = matches!(folder.role, Some(Role::Trash | Role::Spam));
        let account = |kind: &Kind| match kind {
            Kind::Account(part) => Some(email.contains(&part.to_lowercase())),
            _ => None,
        };
        let named = |kind: &Kind| match kind {
            Kind::Folder(name) => Some(folder.answers_to(name)),
            _ => None,
        };
        self.admits(account, true) && self.admits(named, !shunned)
    }

    /// Whether the terms `test` judges let a thing in: one not negated
    /// saying yes, else `otherwise` when there is none, and no negated one
    /// saying yes.
    fn admits(&self, test: impl Fn(&Kind) -> Option<bool>, otherwise: bool) -> bool {
        let mut wanted = None;
        for term in &self.terms {
            match (test(&term.kind), term.negated) {
                (Some(true), true) => return false,
                (Some(yes), false) => wanted = Some(wanted.unwrap_or(false) || yes),
                _ => {}
            }
        }

        wanted.unwrap_or(otherwise)
    }
}

/// A known operator whose value does not parse, as `before:2024-1` while it
/// is typed, is no term; an unknown one is searched as words, name and all.
fn term(token: &Token<'_>, now: DateTime<Local>) -> Option<Term> {
    let (start, quoted) = (token.start, token.quoted);
    let words =
        |name: Option<&str>| words(Field::Any, quoted, name.into_iter().chain([token.value]));
    let (kind, head) = match token.head {
        Head::Operator(operator) => (value(operator, token, now)?, start..token.value_at),
        Head::Bare => (words(None)?, start..start + usize::from(token.negated)),
        Head::Unknown(name) => (
            words(Some(name))?,
            start..start + usize::from(token.negated),
        ),
    };

    Some(Term {
        negated: token.negated,
        kind,
        head,
    })
}

fn value(operator: Operator, token: &Token<'_>, now: DateTime<Local>) -> Option<Kind> {
    let (value, quoted) = (token.value, token.quoted);
    match operator {
        Operator::From => words(Field::From, quoted, [value]),
        Operator::To | Operator::Cc => words(Field::To, quoted, [value]),
        Operator::Subject => words(Field::Subject, quoted, [value]),
        Operator::Is if value.eq_ignore_ascii_case("unread") => Some(Kind::Unread(true)),
        Operator::Is if value.eq_ignore_ascii_case("read") => Some(Kind::Unread(false)),
        Operator::Has if value.eq_ignore_ascii_case("attachment") => Some(Kind::Attachment),
        Operator::In if !value.is_empty() => Some(Kind::Folder(value.to_owned())),
        Operator::Account if !value.is_empty() => Some(Kind::Account(value.to_owned())),
        Operator::Before => midnight(value).map(Kind::Before),
        Operator::After => midnight(value).map(Kind::After),
        Operator::Older => ago(value, now).map(Kind::Before),
        Operator::Newer => ago(value, now).map(Kind::After),
        _ => None,
    }
}

/// The words of `texts`, cut as the index cuts text.
fn words<'a>(field: Field, quoted: bool, texts: impl IntoIterator<Item = &'a str>) -> Option<Kind> {
    let words: Vec<String> = texts
        .into_iter()
        .flat_map(|text| text.split(|c: char| !c.is_alphanumeric()))
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect();
    // An operator's value of several words, `p.novak`, is one name.
    let phrase = quoted || field != Field::Any;
    (!words.is_empty()).then_some(Kind::Words {
        field,
        words,
        phrase,
    })
}

/// The second local midnight starts `YYYY-MM-DD` or `YYYY/MM/DD`.
fn midnight(value: &str) -> Option<i64> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(value, "%Y/%m/%d"))
        .ok()?;
    let local = date.and_hms_opt(0, 0, 0)?.and_local_timezone(Local);
    Some(local.earliest()?.timestamp())
}

/// The second `value`, as `7d`, `2w`, `3m` or `1y`, before `now`.
fn ago(value: &str, now: DateTime<Local>) -> Option<i64> {
    let unit = value.chars().last()?;
    let count: u32 = value.strip_suffix(unit)?.parse().ok()?;
    let then = match unit.to_ascii_lowercase() {
        'd' => now.checked_sub_days(Days::new(count.into())),
        'w' => now.checked_sub_days(Days::new(u64::from(count) * 7)),
        'm' => now.checked_sub_months(Months::new(count)),
        'y' => now.checked_sub_months(Months::new(count.checked_mul(12)?)),
        _ => None,
    }?;
    Some(then.timestamp())
}
