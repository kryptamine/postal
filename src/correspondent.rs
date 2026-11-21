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
    /// Whether an Account wrote to it.
    pub written: bool,
}

impl Correspondent {
    /// As a Draft's header holds it.
    pub fn mailbox(&self) -> String {
        self.address.parse().map_or_else(
            |_bad| self.address.clone(),
            |address| Mailbox::new(self.name.clone(), address).to_string(),
        )
    }
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
            *latest = Some((row.at, name));
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

    // By address among equals, so the order never changes between runs.
    ranked.sort_by(
        |((a_first, a_written, a), x), ((b_first, b_written, b), y)| {
            (b_first, b_written)
                .cmp(&(a_first, a_written))
                .then(b.total_cmp(a))
                .then_with(|| x.address.cmp(&y.address))
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

    let name = tally
        .named_written
        .or(tally.named_seen)
        .map(|(_, name)| name);
    Correspondent {
        address,
        name,
        account,
        written: tally.written,
    }
}

fn no_reply(address: &str) -> bool {
    let local = address.split('@').next().unwrap_or_default();
    let bare: String = local.chars().filter(|c| !".-_".contains(*c)).collect();
    NO_REPLY.iter().any(|word| bare.contains(word))
}
