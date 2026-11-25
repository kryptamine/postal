use icalendar::parser::{self, Component, Property};
use jiff::{
    ZonedDateTime,
    civil::{Date, DateTime},
    tz::TimeZone,
};
use mail_parser::{Message, MimeHeaders, PartType};

use crate::render::{event_zone, meeting_link, recurrence::Rule};

/// The meeting a Message's `text/calendar` part carries, as its card shows it.
pub struct Invitation {
    pub kind: Kind,
    pub title: String,
    pub when: Option<When>,
    pub repeats: Option<String>,
    /// `LOCATION` without the link it may hold, which `join` takes.
    pub location: Option<String>,
    pub join: Option<String>,
    pub organizer: Option<String>,
    /// The Account's own `PARTSTAT`, when it is one of the attendees.
    pub answer: Option<Answer>,
}

/// What the part's `METHOD` makes of it.
pub enum Kind {
    Request,
    Cancel,
    /// An attendee's answer to the user's own invitation.
    Reply {
        attendee: String,
        answer: Answer,
    },
}

pub enum When {
    /// Each in the event's own zone.
    Timed {
        start: ZonedDateTime,
        end: Option<ZonedDateTime>,
    },
    /// The first and the last day, both in.
    AllDay { first: Date, last: Date },
}

#[derive(Clone, Copy)]
pub enum Answer {
    Accepted,
    Declined,
    Tentative,
    Pending,
}

impl Invitation {
    /// The invitation `message` carries, for the Account `own`; `None` when
    /// it has no `VEVENT`, or its `METHOD` is one no card shows.
    pub fn find(message: &Message<'_>, own: &str) -> Option<Self> {
        let text = calendar_text(message)?;
        let unfolded = parser::unfold(&text);
        let calendar = parser::read_calendar(&unfolded).ok()?;
        let method = calendar
            .properties
            .iter()
            .find(|p| p.name.as_str().eq_ignore_ascii_case("METHOD"))
            .map(|p| p.val.as_str().trim().to_ascii_uppercase());
        let zones = &calendar.components;
        let events = || zones.iter().filter(|c| c.name.as_str() == "VEVENT");
        // A series sent with its changed occurrences: the series is the one
        // without a `RECURRENCE-ID`.
        let event = events()
            .find(|e| e.find_prop("RECURRENCE-ID").is_none())
            .or_else(|| events().next())?;

        let kind = kind(method.as_deref()?, event)?;
        let own = own.to_lowercase();
        let text = |name: &str| {
            let value = event.find_prop(name)?.val.as_str().trim().to_owned();
            (!value.is_empty()).then_some(value)
        };

        let location = text("LOCATION");
        let join = join(event, location.as_deref());

        Some(Self {
            kind,
            title: text("SUMMARY").unwrap_or_else(|| "(no title)".to_owned()),
            when: when(event, zones),
            repeats: event
                .find_prop("RRULE")
                .map(|rule| Rule::parse(rule.val.as_str()).line()),
            location: location
                .map(|l| meeting_link::without_urls(&l))
                .filter(|l| !l.is_empty()),
            join,
            organizer: event.find_prop("ORGANIZER").map(|organizer| {
                let address = address(organizer);
                param(organizer, "CN")
                    .map_or_else(|| address.to_owned(), |cn| format!("{cn} <{address}>"))
            }),
            answer: attendees(event)
                .find(|a| address(a).to_lowercase() == own)
                .map(answer),
        })
    }
}

fn kind(method: &str, event: &Component<'_>) -> Option<Kind> {
    match method {
        "REQUEST" => Some(Kind::Request),
        "CANCEL" => Some(Kind::Cancel),
        "REPLY" => {
            let attendee = attendees(event).next()?;
            Some(Kind::Reply {
                attendee: param(attendee, "CN")
                    .map_or_else(|| address(attendee).to_owned(), str::to_owned),
                answer: answer(attendee),
            })
        }
        _ => None,
    }
}

fn attendees<'a>(event: &'a Component<'_>) -> impl Iterator<Item = &'a Property<'a>> {
    let all = event.properties.iter();
    all.filter(|p| p.name.as_str() == "ATTENDEE")
}

/// Any link in `location`, else the description's link to a video call.
fn join(event: &Component<'_>, location: Option<&str>) -> Option<String> {
    let found = location
        .and_then(|l| meeting_link::urls(l).next())
        .or_else(|| {
            let description = event.find_prop("DESCRIPTION")?.val.as_str();
            meeting_link::urls(description).find(|url| meeting_link::meeting(url))
        });

    found.map(str::to_owned)
}

/// The text of the first `text/calendar` part, else of the first
/// `application/ics` one, attached or not.
fn calendar_text(message: &Message<'_>) -> Option<String> {
    let of = |media: &str, subtype: &str| {
        message.parts.iter().find(|part| {
            part.content_type().is_some_and(|t| {
                t.ctype().eq_ignore_ascii_case(media)
                    && t.subtype().is_some_and(|s| s.eq_ignore_ascii_case(subtype))
            })
        })
    };

    let part = of("text", "calendar").or_else(|| of("application", "ics"))?;
    match &part.body {
        PartType::Text(text) => Some(text.to_string()),
        PartType::Binary(bytes) | PartType::InlineBinary(bytes) => {
            Some(String::from_utf8_lossy(bytes).into_owned())
        }
        _ => None,
    }
}

fn param<'a>(property: &'a Property<'_>, key: &str) -> Option<&'a str> {
    let found = property
        .params
        .iter()
        .find(|p| p.key.as_str().eq_ignore_ascii_case(key));
    let value = found?.val.as_ref()?.as_str().trim_matches('"');
    (!value.is_empty()).then_some(value)
}

/// An `ATTENDEE` or `ORGANIZER` value without its `mailto:`.
fn address<'a>(property: &'a Property<'_>) -> &'a str {
    let value = property.val.as_str().trim();
    let scheme = value.get(..7).filter(|s| s.eq_ignore_ascii_case("mailto:"));
    scheme.and_then(|_| value.get(7..)).unwrap_or(value)
}

fn answer(attendee: &Property<'_>) -> Answer {
    match param(attendee, "PARTSTAT")
        .map(str::to_ascii_uppercase)
        .as_deref()
    {
        Some("ACCEPTED") => Answer::Accepted,
        Some("DECLINED") => Answer::Declined,
        Some("TENTATIVE") => Answer::Tentative,
        _ => Answer::Pending,
    }
}

fn when(event: &Component<'_>, zones: &[Component<'_>]) -> Option<When> {
    let start = event.find_prop("DTSTART")?;
    if let Some(first) = date(start) {
        // `DTEND` of an all-day event is the day after its last.
        let last = event
            .find_prop("DTEND")
            .and_then(date)
            .and_then(|end| end.yesterday().ok())
            .filter(|last| *last > first)
            .unwrap_or(first);
        return Some(When::AllDay { first, last });
    }

    Some(When::Timed {
        start: moment(start, zones)?,
        end: event.find_prop("DTEND").and_then(|end| moment(end, zones)),
    })
}

/// A `VALUE=DATE` property, `20261224`.
fn date(property: &Property<'_>) -> Option<Date> {
    let value = property.val.as_str().trim();
    let is_date = param(property, "VALUE").is_some_and(|v| v.eq_ignore_ascii_case("DATE"))
        || value.len() == 8;
    is_date
        .then(|| Date::strptime("%Y%m%d", value).ok())
        .flatten()
}

/// A date and time in UTC (`…Z`), in the zone its `TZID` names, else in the
/// local zone, as RFC 5545 reads a floating time.
fn moment(property: &Property<'_>, zones: &[Component<'_>]) -> Option<ZonedDateTime> {
    let value = property.val.as_str().trim();
    let (civil, utc) = value
        .strip_suffix(['Z', 'z'])
        .map_or((value, false), |v| (v, true));
    let at = DateTime::strptime("%Y%m%dT%H%M%S", civil).ok()?;
    let zone = if utc {
        TimeZone::UTC
    } else {
        param(property, "TZID")
            .and_then(|tzid| event_zone::of(tzid, zones, at))
            .unwrap_or_else(TimeZone::system)
    };

    at.to_zoned_datetime(zone).ok()
}
