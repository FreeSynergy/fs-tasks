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

/// Take the `COMMENT` properties of a `VTODO` out of the task's iCalendar record and give their texts, in order. A comment
/// is its own object, not a property of the task, so the task does not keep them (the original text does).
pub fn take_comments(task: &mut Task) -> Vec<String> {
    let mut texts = Vec::new();
    let Some(props) = task
        .extra
        .get_mut("iCalendar")
        .and_then(|i| i.get_mut("properties"))
        .and_then(serde_json::Value::as_array_mut)
    else {
        return texts;
    };
    props.retain(|p| {
        let is_comment = p
            .get(0)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|n| n.eq_ignore_ascii_case("comment"));
        if is_comment && let Some(t) = p.get(3).and_then(serde_json::Value::as_str) {
            texts.push(t.to_string());
        }
        !is_comment
    });
    texts
}

/// Write comment texts as `COMMENT` properties (export); the task given is a copy for the export.
pub fn put_comments(task: &mut Task, texts: &[String]) {
    if texts.is_empty() {
        return;
    }
    let ical = task
        .extra
        .entry("iCalendar".to_string())
        .or_insert_with(|| serde_json::json!({"name": "vtodo"}));
    if let Some(obj) = ical.as_object_mut() {
        let props = obj
            .entry("properties")
            .or_insert_with(|| serde_json::json!([]));
        if let Some(list) = props.as_array_mut() {
            for t in texts {
                list.push(serde_json::json!(["comment", {}, "unknown", t]));
            }
        }
    }
}
