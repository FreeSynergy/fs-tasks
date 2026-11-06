//! iCalendar `VTODO`s as typed tasks.
//!
//! Pattern: Adapter — `fs-jscalendar` does the conversion on JSON values; this file picks our
//! component kind and our type. `VEVENT`s of the same text belong to `fs-calendar`.

use fs_jscalendar::ical::{self, Component};
pub use fs_jscalendar::ical::{ICS_MEDIA_TYPE, IcsPart};

use crate::error::Result;
use crate::model::Task;

/// Split an `.ics` text into per-UID parts of `VTODO` components.
pub fn split_ics(text: &str) -> Vec<IcsPart> {
    ical::split_ics(text, Component::Task)
}

/// Convert one iCalendar text to upgraded JSCalendar 2.0 tasks (the `VTODO` components; other
/// components are ignored) plus upgrade notes.
pub fn parse_ics(text: &str) -> Result<(Vec<Task>, Vec<String>)> {
    let (entries, notes) = ical::parse_entries(text, Task::TYPE)?;
    let tasks = entries
        .into_iter()
        .map(Task::from_value)
        .collect::<fs_jscalendar::Result<_>>()?;
    Ok((tasks, notes))
}

/// Export tasks as one iCalendar stream of `VTODO`s.
pub fn to_ics(tasks: &[Task]) -> Result<String> {
    Ok(ical::write_ics(
        tasks.iter().map(Task::to_value),
        "-//FreeSynergy//fs-tasks-core//EN",
    )?)
}
