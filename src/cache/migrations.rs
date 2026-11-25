//! The Cache's schema as the steps every released one took from an empty
//! file: see docs/adr/0021-migrate-stored-data-across-releases.md.

use rusqlite::Connection;

/// Each in turn, from the first released schema. A released step never
/// changes: a schema change appends one.
const MIGRATIONS: &[&str] = &[
    include_str!("migrations/1.sql"),
    include_str!("migrations/2.sql"),
    include_str!("migrations/3.sql"),
    include_str!("migrations/4.sql"),
    include_str!("migrations/5.sql"),
    include_str!("migrations/6.sql"),
];

/// The `user_version` of the last schema before migrations; a Cache at or
/// under it is rebuilt.
pub const PRE_RELEASE: i64 = 27;

/// The `user_version` once every step has run.
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_wrap,
    reason = "a const has no `i64::try_from`, and the steps are few"
)]
pub const VERSION: i64 = PRE_RELEASE + MIGRATIONS.len() as i64;

/// Brings a Cache at `version` to `VERSION`; an empty file is at 0. Each
/// step runs in one transaction with the version it reaches.
pub fn migrate(connection: &mut Connection, version: i64) -> rusqlite::Result<()> {
    for (step, sql) in (PRE_RELEASE + 1..).zip(MIGRATIONS) {
        if step <= version {
            continue;
        }

        let transaction = connection.transaction()?;
        transaction.execute_batch(sql)?;
        transaction.pragma_update(None, "user_version", step)?;
        transaction.commit()?;
        tracing::info!("cache migrated to schema {step}");
    }

    Ok(())
}
