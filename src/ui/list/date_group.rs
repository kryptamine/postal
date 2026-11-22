use std::fmt::Write;

use chrono::{DateTime, Datelike, Days, FixedOffset, Month};

use crate::{render::header_date::month_of, render::stack_str::StackStr};

/// Where a Thread sits in the Message List's run of dated headers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DateGroup {
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,
    Month(Month),
    Year(i32),
}

impl DateGroup {
    /// Weeks start on Monday. A date after today, from a skewed clock,
    /// counts as today.
    pub fn new(received: &DateTime<FixedOffset>, now: &DateTime<FixedOffset>) -> Self {
        let (date, today) = (received.date_naive(), now.date_naive());
        let back = |days| today.checked_sub_days(Days::new(days)).unwrap_or(today);
        let monday = back(u64::from(today.weekday().num_days_from_monday()));
        if date >= today {
            Self::Today
        } else if date >= back(1) {
            Self::Yesterday
        } else if date >= monday {
            Self::ThisWeek
        } else if date >= monday.checked_sub_days(Days::new(7)).unwrap_or(monday) {
            Self::LastWeek
        } else if date.year() == today.year() {
            Self::Month(month_of(date))
        } else {
            Self::Year(date.year())
        }
    }

    pub fn label(self) -> StackStr<16> {
        let mut label = StackStr::new();
        // Every label fits, so the write cannot fail.
        (match self {
            Self::Today => label.write_str("Today"),
            Self::Yesterday => label.write_str("Yesterday"),
            Self::ThisWeek => label.write_str("This Week"),
            Self::LastWeek => label.write_str("Last Week"),
            Self::Month(month) => label.write_str(month.name()),
            Self::Year(year) => write!(label, "{year}"),
        })
        .ok();

        label
    }
}

// Unit-tested directly: the groups hinge on "now", which a pty test
// against a fake server cannot pin down.
#[cfg(test)]
mod tests {
    use chrono::DateTime;

    use super::DateGroup;

    // 2026-11-19 is a Thursday.
    fn group(received: &str) -> String {
        let parse = |s: &str| DateTime::parse_from_rfc3339(s).unwrap();
        DateGroup::new(&parse(received), &parse("2026-11-19T12:00:00+02:00"))
            .label()
            .as_str()
            .to_owned()
    }

    #[test]
    fn each_group_starts_where_the_last_ends() {
        let cases = [
            ("2026-11-19T00:00:00+02:00", "Today"),
            ("2026-11-18T23:59:00+02:00", "Yesterday"),
            ("2026-11-18T00:00:00+02:00", "Yesterday"),
            ("2026-11-17T23:59:00+02:00", "This Week"),
            ("2026-11-16T00:00:00+02:00", "This Week"),
            ("2026-11-15T23:59:00+02:00", "Last Week"),
            ("2026-11-09T00:00:00+02:00", "Last Week"),
            ("2026-11-08T23:59:00+02:00", "November"),
            ("2026-01-01T00:00:00+02:00", "January"),
            ("2025-12-31T23:59:00+02:00", "2025"),
        ];
        for (received, label) in cases {
            assert_eq!(group(received), label, "{received}");
        }
    }

    // On a Monday, yesterday is Sunday of last week, and named Yesterday.
    #[test]
    fn yesterday_wins_over_last_week() {
        let parse = |s: &str| DateTime::parse_from_rfc3339(s).unwrap();
        let monday = parse("2026-11-16T09:00:00Z");
        let sunday = parse("2026-11-15T20:00:00Z");
        assert!(DateGroup::new(&sunday, &monday) == DateGroup::Yesterday);
    }

    #[test]
    fn a_skewed_clock_is_today() {
        assert_eq!(group("2026-11-20T09:00:00+02:00"), "Today");
    }
}
