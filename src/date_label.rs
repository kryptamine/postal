use std::fmt::{self, Write};

use chrono::{DateTime, Datelike, FixedOffset, TimeZone, Timelike};

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The date column, formatted on the stack so drawing a frame does not allocate.
pub struct DateLabel {
    bytes: [u8; Self::WIDTH],
    len: usize,
}

impl DateLabel {
    /// The widest form, `2024-11-17`.
    pub const WIDTH: usize = 10;

    /// Today `14:05`, this year `17 Nov`, older `2024-11-17`, in `now`'s time zone.
    pub fn new<Tz: TimeZone>(received: &DateTime<FixedOffset>, now: &DateTime<Tz>) -> Self {
        let local = received.with_timezone(&now.timezone());
        let mut label = Self {
            bytes: [0; Self::WIDTH],
            len: 0,
        };
        // Every form fits WIDTH, so the write cannot fail.
        let _ = if local.date_naive() == now.date_naive() {
            write!(label, "{:02}:{:02}", local.hour(), local.minute())
        } else if local.year() == now.year() {
            write!(label, "{} {}", local.day(), MONTHS[local.month0() as usize])
        } else {
            write!(
                label,
                "{:04}-{:02}-{:02}",
                local.year(),
                local.month(),
                local.day()
            )
        };
        label
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).unwrap_or_default()
    }
}

impl Write for DateLabel {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        self.bytes
            .get_mut(self.len..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
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
    fn today_shows_the_time_in_the_local_zone() {
        assert_eq!(
            label("2026-11-17T10:05:00Z", "2026-11-17T23:00:00+04:00"),
            "14:05"
        );
    }

    #[test]
    fn yesterday_in_the_local_zone_is_not_today() {
        assert_eq!(
            label("2026-11-16T22:30:00Z", "2026-11-17T09:00:00+00:00"),
            "16 Nov"
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
