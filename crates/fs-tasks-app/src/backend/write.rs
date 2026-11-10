//! The values of a form merged into the JSCalendar JSON of a task.
//!
//! Pattern: Adapter, the writing half of the mapping (`read.rs` is the other). The rule of `change` (ADR 0066) lives
//! here and nowhere else: a field that is not in `input` is not touched, an empty list removes the member, values
//! replace the old ones. Members without a field are never touched, so nothing a person did not see is lost.

use fs_admin_frontend::Values;
use fs_tasks_core::time::fmt_utc;
use fs_types::{RecordIssue, TypeProblem};
use serde_json::{Map, Value, json};

use super::instants::{local_of, zone};
use super::issues::issue;

type Object = Map<String, Value>;

fn given(input: &Values, name: &str) -> Option<Vec<String>> {
    input.get(name).map(|v| {
        v.iter()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .collect()
    })
}

fn set_text(obj: &mut Object, name: &str, values: &[String]) {
    match values.first() {
        Some(v) => obj.insert(name.to_owned(), json!(v)),
        None => obj.remove(name),
    };
}

fn set_number(
    obj: &mut Object,
    name: &str,
    values: &[String],
    max: i32,
    problems: &mut Vec<RecordIssue>,
) {
    match values.first().map(|v| {
        v.parse::<i32>()
            .ok()
            .filter(|n| (0..=max).contains(n))
            .ok_or(())
    }) {
        None => {
            obj.remove(name);
        }
        Some(Ok(n)) => {
            obj.insert(name.to_owned(), json!(n));
        }
        Some(Err(_)) => problems.push(issue(name, TypeProblem::Character)),
    }
}

/// A map member whose objects hold one text (`coordinates`, `href`): the objects with that text are kept as they are,
/// objects without it stay, the other texts get a new object.
fn set_objects(obj: &mut Object, name: &str, kind: &str, member: &str, values: &[String]) {
    let mut map = obj
        .remove(name)
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    map.retain(|_, o| {
        o.get(member)
            .and_then(Value::as_str)
            .is_none_or(|t| values.iter().any(|v| v == t))
    });
    for text in values {
        let known = map
            .values()
            .any(|o| o.get(member).and_then(Value::as_str) == Some(text));
        if !known {
            let key = (1..)
                .map(|i| format!("{}{i}", &name[..1]))
                .find(|k| !map.contains_key(k))
                .unwrap_or_default();
            map.insert(key, json!({"@type": kind, member: text}));
        }
    }
    if !map.is_empty() {
        obj.insert(name.to_owned(), Value::Object(map));
    }
}

fn set_keywords(obj: &mut Object, values: &[String]) {
    let words: Vec<&str> = values
        .iter()
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        obj.remove("keywords");
    } else {
        obj.insert(
            "keywords".into(),
            Value::Object(
                words
                    .iter()
                    .map(|w| ((*w).to_owned(), json!(true)))
                    .collect(),
            ),
        );
    }
}

/// Merge `input` into the task JSON. `Err` lists the values that cannot be written.
///
/// # Errors
/// The findings for values that are not a number or not an instant.
pub fn merge(task: &mut Value, input: &Values) -> Result<(), Vec<RecordIssue>> {
    let Some(obj) = task.as_object_mut() else {
        return Ok(());
    };
    let mut problems = Vec::new();
    for name in [
        "title",
        "description",
        "estimatedDuration",
        "color",
        "timeZone",
    ] {
        if let Some(v) = given(input, name) {
            set_text(obj, name, &v);
        }
    }
    // after the zone: the instants are read in the zone the task has now
    let zone = zone(obj.get("timeZone").and_then(Value::as_str));
    for name in ["start", "due"] {
        let Some(v) = given(input, name) else {
            continue;
        };
        match v.first().map(|t| local_of(t, &zone)) {
            None => set_text(obj, name, &[]),
            Some(Some(local)) => set_text(obj, name, &[local]),
            Some(None) => problems.push(issue(name, TypeProblem::BadDateTime)),
        }
    }
    if let Some(v) = given(input, "progress") {
        let before = obj.get("progress").cloned();
        set_text(obj, "progress", &v);
        if obj.get("progress") != before.as_ref() {
            obj.insert("progressUpdated".into(), json!(fmt_utc(chrono::Utc::now())));
        }
    }
    if let Some(v) = given(input, "percentComplete") {
        set_number(obj, "percentComplete", &v, 100, &mut problems);
    }
    if let Some(v) = given(input, "priority") {
        set_number(obj, "priority", &v, 9, &mut problems);
    }
    if let Some(v) = given(input, "keywords") {
        set_keywords(obj, &v);
    }
    if let Some(v) = given(input, "locations") {
        set_objects(obj, "locations", "Location", "coordinates", &v);
    }
    if let Some(v) = given(input, "links") {
        set_objects(obj, "links", "Link", "href", &v);
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}
