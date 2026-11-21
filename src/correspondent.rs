use std::collections::HashMap;

use chrono::{DateTime, Utc};
use lettre::message::Mailbox;

use crate::cache::Seen;

/// Days over which one Message's weight in a rank halves.
const HALF_LIFE_DAYS: f64 = 90.0;

/// Local parts, `.`, `-` and `_` left out, of addresses no one reads.
const NO_REPLY: [&str; 3] = ["noreply", "donotreply", "mailerdaemon"];

/// A Correspondent as a Draft's recipients are expanded to.
#[derive(Clone)]
pub struct Correspondent {
    pub address: String,
    pub name: Option<String>,
    /// The address of the Account that knows it best.
    pub account: String,
    /// Each word of its names and of its address, lowercased.
    words: Vec<String>,
}

impl Correspondent {
    /// Whether every word of `query` begins one of its words.
    pub fn matches(&self, query: &[String]) -> bool {
        query.iter().all(|asked| {
            self.words
                .iter()
                .any(|word| word.starts_with(asked.as_str()))
        })
    }

    /// As a Draft's header holds it.
    pub fn mailbox(&self) -> String {
        self.address.parse().map_or_else(
            |_bad| self.address.clone(),
            |address| Mailbox::new(self.name.clone(), address).to_string(),
        )
    }
}

/// The lowercased words of `text` a Query is matched against.
pub fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| c.is_whitespace() || "\"(),<>".contains(c))
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
}

/// What the rows of one address add up to.
#[derive(Default)]
struct Tally {
    written: bool,
    score: f64,
    /// By Account, so the one that knows it best is named.
    scores: HashMap<String, f64>,
    /// The latest name written to, and the latest seen, with when.
    named_written: Option<(DateTime<Utc>, String)>,
    named_seen: Option<(DateTime<Utc>, String)>,
    names: Vec<String>,
}

/// Every Correspondent of `seen` but the Accounts' own addresses and a
/// no-reply one never written to: the Account `first`'s first, then those
/// written to, then by how often and how lately at `now`.
pub fn ranked(
    seen: Vec<Seen>,
    (accounts, first): (&[&str], &str),
    now: DateTime<Utc>,
) -> Vec<Correspondent> {
    let mut tallies: HashMap<String, Tally> = HashMap::new();
    for row in seen {
        let tally = tallies.entry(row.address).or_default();
        let hours = i32::try_from((now - row.at).num_hours().max(0)).unwrap_or(i32::MAX);
        let weight = 0.5_f64.powf(f64::from(hours) / 24.0 / HALF_LIFE_DAYS);
        tally.written |= row.written;
        tally.score += weight;
        *tally.scores.entry(row.account).or_default() += weight;
        let Some(name) = row.name else {
            continue;
        };

        let latest = if row.written {
            &mut tally.named_written
        } else {
            &mut tally.named_seen
        };

        if latest.as_ref().is_none_or(|(at, _)| *at <= row.at) {
            *latest = Some((row.at, name.clone()));
        }

        if !tally.names.contains(&name) {
            tally.names.push(name);
        }
    }

    let first = first.to_lowercase();
    let mut ranked: Vec<_> = tallies
        .into_iter()
        .filter(|(address, tally)| {
            let own = accounts.iter().any(|a| a.eq_ignore_ascii_case(address));
            !own && (tally.written || !no_reply(address))
        })
        .map(|(address, tally)| {
            let of_first = tally.scores.contains_key(&first);
            let rank = (of_first, tally.written, tally.score);
            (rank, correspondent(address, tally, &first))
        })
        .collect();

    ranked.sort_by(
        |((a_first, a_written, a), _), ((b_first, b_written, b), _)| {
            (b_first, b_written)
                .cmp(&(a_first, a_written))
                .then(b.total_cmp(a))
        },
    );

    ranked.into_iter().map(|(_, c)| c).collect()
}

fn correspondent(address: String, tally: Tally, first: &str) -> Correspondent {
    let account = if tally.scores.contains_key(first) {
        first.to_owned()
    } else {
        let best = tally.scores.iter().max_by(|(_, a), (_, b)| a.total_cmp(b));
        best.map(|(account, _)| account.clone()).unwrap_or_default()
    };

    let (local, domain) = address.split_once('@').unwrap_or((&address, ""));
    let mut words: Vec<String> = tally.names.iter().flat_map(|n| words(n)).collect();
    words.push(address.clone());
    words.extend(local.split(['.', '-', '_', '+']).map(str::to_owned));
    words.push(domain.to_owned());
    words.extend(domain.split('.').map(str::to_owned));
    words.retain(|word| !word.is_empty());
    let name = tally
        .named_written
        .or(tally.named_seen)
        .map(|(_, name)| name);
    Correspondent {
        address,
        name,
        account,
        words,
    }
}

fn no_reply(address: &str) -> bool {
    let local = address.split('@').next().unwrap_or_default();
    let bare: String = local.chars().filter(|c| !".-_".contains(*c)).collect();
    NO_REPLY.iter().any(|word| bare.contains(word))
}
