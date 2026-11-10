//! Days for the views: the day a stored local time falls on, in the zone of its task.
//!
//! Pattern: none — two small pure functions.

use chrono::Datelike;
use fs_components_calendar_grid::Day;
use fs_tasks_core::time::parse_local;

/// Today (UTC), as the components count days.
pub fn today() -> Day {
    of_date(chrono::Utc::now().date_naive())
}

/// A calendar date as a day.
pub fn of_date(d: chrono::NaiveDate) -> Day {
    Day::from_ymd(
        d.year(),
        u8::try_from(d.month()).unwrap_or(1),
        u8::try_from(d.day()).unwrap_or(1),
    )
}

/// The day of a `start` or `due` of the task (a local time: the day written is the day shown).
pub fn day_of(local: Option<&str>) -> Option<Day> {
    parse_local(local?).ok().map(|t| of_date(t.date()))
}
