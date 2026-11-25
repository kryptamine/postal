use icalendar::parser::Component;
use jiff::{
    civil::{Date, DateTime},
    tz::{Offset, TimeZone},
};

use crate::{
    render::recurrence::{self, Rule},
    render::windows_zones,
};

/// The zone a `TZID` names: an IANA name, else a Windows one as Outlook
/// sends, else the `VTIMEZONE` of that id among `zones` as it stands at `at`.
pub fn of(tzid: &str, zones: &[Component<'_>], at: DateTime) -> Option<TimeZone> {
    let name = tzid.trim_matches('"');
    TimeZone::get(name)
        .ok()
        .or_else(|| windows_zones::iana(name).and_then(|iana| TimeZone::get(iana).ok()))
        .or_else(|| defined(name, zones, at))
}

/// The offset of the latest `STANDARD` or `DAYLIGHT` onset of the
/// `VTIMEZONE` `tzid` before `at`, as a fixed zone.
fn defined(tzid: &str, zones: &[Component<'_>], at: DateTime) -> Option<TimeZone> {
    let zone = zones.iter().find(|zone| {
        zone.name.as_str() == "VTIMEZONE"
            && zone
                .find_prop("TZID")
                .is_some_and(|id| id.val.as_str().trim_matches('"') == tzid)
    })?;

    let latest = zone
        .components
        .iter()
        .filter_map(|rule| {
            let to = offset(rule.find_prop("TZOFFSETTO")?.val.as_str())?;
            let start =
                DateTime::strptime("%Y%m%dT%H%M%S", rule.find_prop("DTSTART")?.val.as_str())
                    .ok()?;
            Some((onset(rule, start, at)?, to))
        })
        .max_by_key(|(onset, _)| *onset)?;
    Some(TimeZone::fixed(latest.1))
}

/// The latest time the rule `rule`, first in force at `start`, took effect
/// at or before `at`.
fn onset(rule: &Component<'_>, start: DateTime, at: DateTime) -> Option<DateTime> {
    let Some(yearly) = rule.find_prop("RRULE") else {
        return (start <= at).then_some(start);
    };

    let yearly = Rule::parse(yearly.val.as_str());
    [at.year(), at.year() - 1]
        .into_iter()
        .filter_map(|year| in_year(&yearly, start, year))
        .find(|onset| *onset <= at && *onset >= start)
}

/// When a yearly rule, `BYMONTH=3;BYDAY=-1SU`, takes effect in `year`.
fn in_year(rule: &Rule<'_>, start: DateTime, year: i16) -> Option<DateTime> {
    let month = rule
        .get("BYMONTH")
        .and_then(|m| m.parse().ok())
        .unwrap_or_else(|| start.month());
    let date = match rule.get("BYDAY").and_then(recurrence::by_day) {
        Some((nth, weekday)) => Date::new(year, month, 1)
            .ok()?
            .nth_weekday_of_month(nth.unwrap_or(1), weekday)
            .ok()?,
        None => Date::new(year, month, start.day()).ok()?,
    };

    Some(date.to_datetime(start.time()))
}

/// `+0100`, `-0800` or `+053000` as an offset.
fn offset(value: &str) -> Option<Offset> {
    let (sign, digits) = match value.get(..1)? {
        "-" => (-1, value.get(1..)?),
        "+" => (1, value.get(1..)?),
        _ => (1, value),
    };

    let part = |range: std::ops::Range<usize>| -> Option<i32> {
        digits.get(range).map_or(Some(0), |d| d.parse().ok())
    };

    let seconds = part(0..2)? * 3600 + part(2..4)? * 60 + part(4..6)?;
    Offset::from_seconds(sign * seconds).ok()
}
