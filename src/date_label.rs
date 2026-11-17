use std::fmt::Write;

use chrono::{DateTime, Datelike, FixedOffset, Timelike};

use crate::stack_str::StackStr;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The widest form, `2024-11-17`.
const WIDTH: usize = 10;

/// The date column, formatted on the stack so drawing a frame does not allocate.
pub struct DateLabel(StackStr<WIDTH>);

impl DateLabel {
    /// Today `14:05`, this year `17 Nov`, older `2024-11-17`. Both times are
    /// local already, each at the offset in force at its own instant.
    pub fn new(received: &DateTime<FixedOffset>, now: &DateTime<FixedOffset>) -> Self {
        let mut label = StackStr::new();
        // Every form fits WIDTH, so the write cannot fail.
        let _ = if received.date_naive() == now.date_naive() {
            write!(label, "{:02}:{:02}", received.hour(), received.minute())
        } else if received.year() == now.year() {
            write!(
                label,
                "{} {}",
                received.day(),
                MONTHS[received.month0() as usize]
            )
        } else {
            write!(
                label,
                "{:04}-{:02}-{:02}",
                received.year(),
                received.month(),
                received.day()
            )
        };

        Self(label)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

// Unit-tested directly: the three forms hinge on "now", which a pty test
// against a fake server cannot pin down.
#[cfg(test)]
mod tests {
    use chrono::DateTime;

    use super::DateLabel;

    fn label(received: &str, now: &str) -> String {
        let parse = |s: &str| DateTime::parse_from_rfc3339(s).unwrap();
        DateLabel::new(&parse(received), &parse(now))
            .as_str()
            .to_owned()
    }

    #[test]
    fn today_shows_the_time() {
        assert_eq!(
            label("2026-11-17T14:05:00+04:00", "2026-11-17T23:00:00+04:00"),
            "14:05"
        );
    }

    #[test]
    fn yesterday_is_not_today() {
        assert_eq!(
            label("2026-11-16T22:30:00Z", "2026-11-17T09:00:00+00:00"),
            "16 Nov"
        );
    }

    // A summer Message keeps its summer offset in winter, or its date would
    // move back a day when it arrived just after midnight.
    #[test]
    fn each_time_keeps_its_own_offset() {
        assert_eq!(
            label("2026-07-01T00:30:00+02:00", "2026-11-17T12:00:00+01:00"),
            "1 Jul"
        );
    }

    #[test]
    fn this_year_shows_day_and_month() {
        assert_eq!(
            label("2026-03-02T12:00:00Z", "2026-11-17T12:00:00Z"),
            "2 Mar"
        );
    }

    #[test]
    fn older_shows_the_full_date() {
        assert_eq!(
            label("2024-06-15T12:00:00Z", "2026-11-17T12:00:00Z"),
            "2024-06-15"
        );
    }
}
