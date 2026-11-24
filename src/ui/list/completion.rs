use std::sync::Arc;

use crate::{
    cache::{self, Cache, Correspondents},
    domain::operator::Operator,
    domain::per_account::PerAccount,
    domain::query_token::{Head, tokens},
};

/// The most suggestions the menu shows of Correspondents and Folders.
const MOST: usize = 8;

/// The menu over the Query line: what the Query's last word can become.
pub struct Completion {
    /// Where the word completed starts in the Query: what a suggestion replaces.
    at: usize,
    suggestions: Vec<Suggestion>,
    /// The suggestion written into the Query, once `Tab` has entered the menu.
    selected: Option<usize>,
    correspondents: Correspondents,
    /// Each Account's address.
    accounts: PerAccount<Arc<str>>,
}

pub struct Suggestion {
    /// Written into the Query.
    pub text: String,
    /// Shown muted beside it.
    pub detail: String,
}

impl Completion {
    /// Starts reading the Correspondents, done before a word asks for them.
    pub fn new(caches: &PerAccount<Cache>, accounts: PerAccount<Arc<str>>) -> Self {
        // Every Account's rows sit in the one store all handles share.
        let correspondents = caches.values().next().map(Cache::correspondents);
        Self {
            at: 0,
            suggestions: Vec::new(),
            selected: None,
            correspondents: correspondents.unwrap_or_default(),
            accounts,
        }
    }

    pub fn suggestions(&self) -> &[Suggestion] {
        &self.suggestions
    }

    pub const fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Where in the Query the menu's word starts.
    pub const fn at(&self) -> usize {
        self.at
    }

    /// Suggests anew for the last word of `query`: every operator for an
    /// empty Query, none after a space or a closing quote.
    pub fn suggest(&mut self, query: &str, caches: &PerAccount<Cache>) -> Result<(), cache::Error> {
        self.close();
        let last = tokens(query).last();
        let open = last.as_ref().is_some_and(|token| token.open);
        if query.ends_with(char::is_whitespace) && !open {
            return Ok(());
        }

        let Some(token) = last else {
            self.at = 0;
            self.operators("");
            return Ok(());
        };

        if token.quoted && !open {
            return Ok(());
        }

        self.at = token.value_at;
        let value = token.value.to_lowercase();
        match token.head {
            Head::Bare => self.operators(&value),
            Head::Operator(Operator::From | Operator::To | Operator::Cc) => self.people(&value),
            Head::Operator(Operator::In) => self.folders(&value, caches)?,
            Head::Operator(Operator::Account) => {
                let emails: Vec<_> = (self.accounts.values())
                    .map(|email| (email.to_string(), ""))
                    .collect();
                self.offer(&value, emails);
            }
            Head::Operator(Operator::Is) => {
                let values = [("unread", "not yet read"), ("read", "read already")];
                self.offer(&value, values.map(|(v, d)| (v.to_owned(), d)));
            }
            Head::Operator(Operator::Has) => {
                self.offer(&value, [("attachment".to_owned(), "one or more")]);
            }
            Head::Operator(_) | Head::Unknown(_) => {}
        }

        // A word already whole needs no menu.
        if let [only] = &*self.suggestions
            && only.text.eq_ignore_ascii_case(token.value)
        {
            self.suggestions.clear();
        }

        Ok(())
    }

    /// Writes the next suggestion, or the previous one, into `query` in
    /// place of the word; whether there was one.
    pub fn cycle(&mut self, query: &mut String, forward: bool) -> bool {
        let count = self.suggestions.len();
        if count == 0 {
            return false;
        }

        let next = match (self.selected, forward) {
            (None, true) => 0,
            (None, false) => count - 1,
            (Some(at), true) => (at + 1) % count,
            (Some(at), false) => (at + count - 1) % count,
        };

        self.selected = Some(next);
        query.truncate(self.at);
        let text = &self.suggestions[next].text;
        if text.contains(char::is_whitespace) {
            query.push('"');
            query.push_str(text);
            query.push('"');
        } else {
            query.push_str(text);
        }

        true
    }

    /// Whether it was open.
    pub fn close(&mut self) -> bool {
        let open = !self.suggestions.is_empty();
        self.suggestions.clear();
        self.selected = None;
        open
    }

    /// Every operator whose name starts with `prefix`, past `MOST`: an
    /// empty Query lists them all.
    fn operators(&mut self, prefix: &str) {
        let found = Operator::ALL
            .into_iter()
            .filter(|operator| operator.name().starts_with(prefix))
            .map(|operator| Suggestion {
                text: format!("{}:", operator.name()),
                detail: detail(operator).to_owned(),
            });
        self.suggestions.extend(found);
    }

    /// Correspondents whose address, or a word of whose name, starts with
    /// `prefix`.
    fn people(&mut self, prefix: &str) {
        let starts = |text: &str| {
            text.starts_with(prefix)
                || (text.split(|c: char| !c.is_alphanumeric())).any(|word| word.starts_with(prefix))
        };
        let found = (self.correspondents.get().iter())
            .filter(|known| starts(&known.address) || starts(&known.lower))
            .take(MOST)
            .map(|known| Suggestion {
                text: known.address.clone(),
                detail: known.name.clone(),
            });
        self.suggestions.extend(found);
    }

    /// Folders, by their label, whose label holds `part`.
    fn folders(&mut self, part: &str, caches: &PerAccount<Cache>) -> Result<(), cache::Error> {
        let several = caches.values().nth(1).is_some();
        for (account, cache) in caches.iter() {
            for folder in cache.folders()? {
                let label = folder.label();
                if self.suggestions.len() == MOST
                    || !label.to_lowercase().contains(part)
                    || self.suggestions.iter().any(|s| s.text == label)
                {
                    continue;
                }

                let detail = if several {
                    &*self.accounts[account]
                } else {
                    ""
                };
                self.suggestions.push(Suggestion {
                    text: label.to_owned(),
                    detail: detail.to_owned(),
                });
            }
        }

        Ok(())
    }

    /// Those of `offered`, each a text and its detail, whose text starts
    /// with `prefix`.
    fn offer<'a>(&mut self, prefix: &str, offered: impl IntoIterator<Item = (String, &'a str)>) {
        let found = offered
            .into_iter()
            .filter(|(text, _)| text.to_lowercase().starts_with(prefix))
            .map(|(text, detail)| Suggestion {
                text,
                detail: detail.to_owned(),
            });
        self.suggestions.extend(found);
    }
}

const fn detail(operator: Operator) -> &'static str {
    match operator {
        Operator::From => "sender, by name or address",
        Operator::To => "recipient in To or Cc",
        Operator::Cc => "recipient, as to:",
        Operator::Subject => "word of the subject",
        Operator::Is => "unread or read",
        Operator::Has => "attachment",
        Operator::In => "one Folder, Trash and Spam too",
        Operator::Account => "one Account",
        Operator::Before => "arrived before YYYY-MM-DD",
        Operator::After => "arrived on YYYY-MM-DD or later",
        Operator::Older => "arrived over 7d, 2w, 3m, 1y ago",
        Operator::Newer => "arrived within 7d, 2w, 3m, 1y",
    }
}
