use chrono::{DateTime, FixedOffset, Offset, Utc};
use rusqlite::Row;

/// The arrival in the columns `at` and `at + 1`: its second and the offset
/// it was sent at.
pub(super) fn received(row: &Row<'_>, at: usize) -> rusqlite::Result<DateTime<FixedOffset>> {
    let offset = FixedOffset::east_opt(row.get(at + 1)?).unwrap_or_else(|| Utc.fix());
    Ok(DateTime::from_timestamp(row.get(at)?, 0)
        .unwrap_or_default()
        .with_timezone(&offset))
}
