use std::fmt::Write;

use chrono::{DateTime, Datelike, FixedOffset, Month, NaiveDate, Timelike};

use crate::render::stack_str::StackStr;

/// How a header shows the date, `Mon, 18 Nov 2024 09:00`. Written by
/// hand on the stack: chrono's `format` allocates, and a frame draws this.
pub fn date(received: &DateTime<FixedOffset>) -> StackStr<40> {
    let mut date = StackStr::new();
    let month = month_of(received.date_naive()).name();
    write!(
        date,
        "{}, {} {} {} {:02}:{:02}",
        received.weekday(),
        received.day(),
        month.get(..3).unwrap_or(month),
        received.year(),
        received.hour(),
        received.minute()
    )
    .ok();
    date
}

/// chrono numbers a date's month, always 1 to 12, rather than naming it.
pub fn month_of(date: NaiveDate) -> Month {
    u8::try_from(date.month())
        .ok()
        .and_then(|m| Month::try_from(m).ok())
        .unwrap_or(Month::January)
}
