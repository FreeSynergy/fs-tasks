//! What an import could not read, told to a person.
//!
//! Pattern: Specification — one rule per property of a `VTODO`. The conversion keeps a value it cannot
//! read as a raw `iCalendar` property, so the task only lacks the member. Only the component's own
//! lines count, unfolded: an alarm has a `DURATION` and a time zone a `DTSTART` of their own.

use chrono::NaiveDateTime;

use crate::ical::unfolded_lines;
use crate::model::Task;
use crate::time::Dur;

/// The own lines of each `VTODO` in `text`: without its nested components.
fn own_lines(text: &str) -> Vec<Vec<String>> {
    let (mut todos, mut own, mut depth) = (vec![], vec![], 0usize);
    for line in unfolded_lines(text) {
        let upper = line.to_ascii_uppercase();
        if depth == 0 {
            if upper == "BEGIN:VTODO" {
                depth = 1;
            }
        } else if upper.starts_with("BEGIN:") {
            depth += 1;
        } else if upper.starts_with("END:") {
            depth -= 1;
            if depth == 0 {
                todos.push(std::mem::take(&mut own));
            }
        } else if depth == 1 {
            own.push(line);
        }
    }
    todos
}

/// The value of property `name`, if the lines have it.
fn value<'a>(own: &'a [String], name: &str) -> Option<&'a str> {
    own.iter().find_map(|line| {
        let head = line.get(..name.len())?;
        let rest = line[name.len()..].chars().next()?;
        if !head.eq_ignore_ascii_case(name) || !matches!(rest, ':' | ';') {
            return None;
        }
        Some(line.split_once(':').map_or("", |(_, v)| v))
    })
}

/// What the conversion made of the `DURATION` of a `VTODO`: it lands in `duration`.
fn has_duration(t: &Task) -> bool {
    let carried = t.extra.get("duration").and_then(|d| d.as_str());
    t.estimated_duration
        .as_deref()
        .or(carried)
        .is_some_and(|d| Dur::parse(d).is_ok())
}

/// The conversion has no member for `COMPLETED` at all, so only its form tells: a UTC date-time.
fn is_completed_time(value: &str) -> bool {
    NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ").is_ok()
}

/// A note for each property of `text` the conversion left out of its task.
pub(super) fn unread_properties(text: &str, tasks: &[Task]) -> Vec<String> {
    let mut notes = vec![];
    for own in own_lines(text) {
        // An instance of a series is no task of its own; what it says stays in the original.
        if value(&own, "RECURRENCE-ID").is_some() {
            continue;
        }
        let uid = value(&own, "UID");
        let Some(t) = tasks.iter().find(|t| Some(t.uid.as_str()) == uid) else {
            continue;
        };
        let completed = value(&own, "COMPLETED").is_none_or(is_completed_time);
        for (name, read) in [
            ("DTSTART", t.start.is_some()),
            ("DUE", t.due.is_some()),
            ("DURATION", has_duration(t)),
            ("COMPLETED", completed),
            ("PERCENT-COMPLETE", t.percent_complete.is_some()),
            ("RRULE", t.recurrence_rule.is_some()),
        ] {
            if !read && value(&own, name).is_some() {
                notes.push(format!("{}: {name} could not be read", t.uid));
            }
        }
    }
    notes
}
