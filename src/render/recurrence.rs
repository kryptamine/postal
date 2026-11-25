use std::fmt::Write;

use jiff::civil::Weekday;

/// An `RRULE` value, `FREQ=WEEKLY;BYDAY=MO`, by its parts.
pub struct Rule<'a> {
    parts: Vec<(&'a str, &'a str)>,
}

impl<'a> Rule<'a> {
    pub fn parse(value: &'a str) -> Self {
        let parts = value
            .split(';')
            .filter_map(|part| part.split_once('='))
            .collect();
        Self { parts }
    }

    pub fn get(&self, key: &str) -> Option<&'a str> {
        let found = self.parts.iter().find(|(k, _)| k.eq_ignore_ascii_case(key));
        found.map(|(_, value)| *value)
    }

    /// The rule as one plain line, `weekly on Mon, Wed until 1 Mar 2027`;
    /// its occurrences are never expanded.
    pub fn line(&self) -> String {
        let interval = self
            .get("INTERVAL")
            .and_then(|n| n.parse::<u32>().ok())
            .filter(|&n| n > 1);
        let (every, unit) = match self.get("FREQ").map(str::to_ascii_uppercase).as_deref() {
            Some("DAILY") => ("daily", "days"),
            Some("WEEKLY") => ("weekly", "weeks"),
            Some("MONTHLY") => ("monthly", "months"),
            Some("YEARLY") => ("yearly", "years"),
            _ => ("repeating", ""),
        };

        let mut line = match interval {
            Some(n) if !unit.is_empty() => format!("every {n} {unit}"),
            _ => every.to_owned(),
        };

        let days: Vec<String> = self
            .get("BYDAY")
            .into_iter()
            .flat_map(|days| days.split(','))
            .filter_map(day_name)
            .collect();
        // Writing to a String cannot fail.
        if !days.is_empty() {
            write!(line, " on {}", days.join(", ")).ok();
        } else if let Some(day) = self.get("BYMONTHDAY") {
            write!(line, " on day {day}").ok();
        }

        if let Some(count) = self.get("COUNT") {
            write!(line, ", {count} times").ok();
        }

        if let Some(until) = self.get("UNTIL").and_then(until_date) {
            write!(line, " until {until}").ok();
        }

        line
    }
}

/// A `BYDAY` entry, `MO` or `-1SU`: its place in the month, if given, and
/// its weekday.
pub fn by_day(entry: &str) -> Option<(Option<i8>, Weekday)> {
    let split = entry.len().checked_sub(2)?;
    let (nth, code) = (entry.get(..split)?, entry.get(split..)?);
    let weekday = match code.to_ascii_uppercase().as_str() {
        "MO" => Weekday::Monday,
        "TU" => Weekday::Tuesday,
        "WE" => Weekday::Wednesday,
        "TH" => Weekday::Thursday,
        "FR" => Weekday::Friday,
        "SA" => Weekday::Saturday,
        "SU" => Weekday::Sunday,
        _ => return None,
    };

    let nth = if nth.is_empty() {
        None
    } else {
        Some(nth.trim_start_matches('+').parse().ok()?)
    };

    Some((nth, weekday))
}

/// `MO` as `Mon`, `2TU` as `the 2nd Tue`, `-1FR` as `the last Fri`.
fn day_name(entry: &str) -> Option<String> {
    let (nth, weekday) = by_day(entry)?;
    let name = weekday_name(weekday);
    Some(match nth {
        None => name.to_owned(),
        Some(-1) => format!("the last {name}"),
        Some(n) => format!("the {} {name}", ordinal(n)),
    })
}

pub const fn weekday_name(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "Mon",
        Weekday::Tuesday => "Tue",
        Weekday::Wednesday => "Wed",
        Weekday::Thursday => "Thu",
        Weekday::Friday => "Fri",
        Weekday::Saturday => "Sat",
        Weekday::Sunday => "Sun",
    }
}

fn ordinal(n: i8) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };

    format!("{n}{suffix}")
}

/// The date `UNTIL` names, `1 Mar 2027`, from its first eight digits.
fn until_date(until: &str) -> Option<String> {
    let date = jiff::civil::Date::strptime("%Y%m%d", until.get(..8)?).ok()?;
    Some(date.strftime("%-d %b %Y").to_string())
}
