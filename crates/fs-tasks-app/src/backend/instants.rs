//! Date-times between the front end (an instant with an offset) and JSCalendar (a local time in the task's zone).
//!
//! Pattern: Adapter — `fs-types` `DateTime` is an instant; a JSCalendar `start`/`due` is a `LocalDateTime` read in
//! `timeZone` (floating = UTC here). The task keeps its zone; only the local text is written.

use chrono::{DateTime, Utc};
use fs_tasks_core::time::{Zone, fmt_local, fmt_utc, parse_local};

/// The zone of a task; UTC when it has none or a name this build does not know.
pub fn zone(name: Option<&str>) -> Zone {
    name.and_then(|n| Zone::parse(n).ok())
        .unwrap_or_else(|| Zone::parse("UTC").expect("UTC is always known"))
}

/// A stored local time as an instant text (`2026-12-01T09:00:00Z`); `None` if it is not readable.
pub fn instant_of(local: &str, zone: &Zone) -> Option<String> {
    let local = parse_local(local).ok()?;
    Some(fmt_utc(zone.to_utc(local)))
}

/// A typed instant as the local time of `zone`.
pub fn local_of(instant: &str, zone: &Zone) -> Option<String> {
    let utc: DateTime<Utc> = DateTime::parse_from_rfc3339(instant.trim())
        .ok()?
        .with_timezone(&Utc);
    Some(fmt_local(zone.to_local(utc)))
}

/// A typed instant in UTC (for the filters of `find`).
pub fn utc_of(instant: &str) -> Option<DateTime<Utc>> {
    Some(
        DateTime::parse_from_rfc3339(instant.trim())
            .ok()?
            .with_timezone(&Utc),
    )
}
