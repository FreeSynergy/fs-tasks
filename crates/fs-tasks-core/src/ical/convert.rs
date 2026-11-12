//! iCalendar `VTODO`s as typed tasks.
//!
//! Pattern: Adapter — `fs-jscalendar` does the conversion on JSON values; this file picks our
//! component kind and our type. `VEVENT`s of the same text belong to `fs-calendar`.

use fs_jscalendar::ical::{self, Component};
pub use fs_jscalendar::ical::{ICS_MEDIA_TYPE, IcsPart, unfolded_lines};

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
        .map(|v| {
            let mut task = Task::from_value(v)?;
            task.adopt_converted_gaps();
            Ok(task)
        })
        .collect::<fs_jscalendar::Result<_>>()?;
    Ok((tasks, notes))
}

/// Export tasks as one iCalendar stream of `VTODO`s.
pub fn to_ics(tasks: &[Task]) -> Result<String> {
    // The member `freesynergy.net:gap` is the truth; the conversion writes `GAP` from its own record of the parameter.
    let values = tasks.iter().map(|t| {
        let mut t = t.clone();
        let with_gap: Vec<String> = t
            .related_to
            .iter()
            .filter(|(_, v)| v.get(crate::model::GAP_KEY).is_some())
            .map(|(uid, _)| uid.clone())
            .collect();
        for uid in with_gap {
            t.mirror_gap(&uid);
        }
        // An assignee without an e-mail address (a program, a service) goes out as an attendee whose address is its actor
        // IRI (RFC 5545: a calendar user address is a URI), not as nothing.
        for p in t.participants.values_mut() {
            if p.email.is_none()
                && p.calendar_address.is_none()
                && let Some(actor) = crate::model::actor_of(p).map(str::to_string)
            {
                p.calendar_address = Some(actor);
            }
        }
        t.to_value()
    });
    Ok(ical::write_ics(
        values,
        "-//FreeSynergy//fs-tasks-core//EN",
    )?)
}
