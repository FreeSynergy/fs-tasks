//! A stored task as the values of the type description.
//!
//! Pattern: Adapter (JSCalendar JSON -> `Values`), the reading half of the mapping; `write.rs` is the other half.
//! Members the description lists but a person cannot edit (`participants`, `relatedTo`, `recurrenceRules`, `alerts`)
//! are shown; members without a field (`sequence`, `privacy` …) stay in the JSON and are never touched.

use fs_admin_frontend::{Record, Values};
use fs_tasks_core::TaskEntry;
use serde_json::Value;

use super::ids::record_id;
use super::instants::{instant_of, zone};

fn put(values: &mut Values, name: &str, value: Option<String>) {
    if let Some(v) = value.filter(|v| !v.is_empty()) {
        values.insert(name.to_owned(), vec![v]);
    }
}

fn texts(values: &mut Values, name: &str, list: Vec<String>) {
    if !list.is_empty() {
        values.insert(name.to_owned(), list);
    }
}

fn strings(json: &Value, key: &str) -> Option<String> {
    json.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// The member `key` of every object of a JSCalendar map member, as a text each.
fn of_map(json: &Value, key: &str, member: &str) -> Vec<String> {
    json.get(key)
        .and_then(Value::as_object)
        .map(|m| {
            m.values()
                .filter_map(|o| o.get(member).and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The record of a stored task.
pub fn record(entry: &TaskEntry) -> Record {
    let json = entry.task.to_value();
    let zone = zone(entry.task.time_zone.as_deref());
    let mut v = Values::new();
    put(&mut v, "title", strings(&json, "title"));
    put(&mut v, "description", strings(&json, "description"));
    for key in ["start", "due"] {
        let local = strings(&json, key);
        put(&mut v, key, local.and_then(|l| instant_of(&l, &zone)));
    }
    put(
        &mut v,
        "estimatedDuration",
        strings(&json, "estimatedDuration"),
    );
    put(
        &mut v,
        "progress",
        Some(entry.task.effective_progress().to_owned()),
    );
    put(
        &mut v,
        "percentComplete",
        json.get("percentComplete").map(Value::to_string),
    );
    put(
        &mut v,
        "priority",
        json.get("priority").map(Value::to_string),
    );
    let keywords: Vec<&str> = entry
        .task
        .keywords
        .iter()
        .filter(|(_, on)| **on)
        .map(|(k, _)| k.as_str())
        .collect();
    put(&mut v, "keywords", Some(keywords.join(", ")));
    let people = entry
        .task
        .participants
        .values()
        .filter_map(|p| p.email.as_deref().map(|e| format!("mailto:{e}")));
    texts(&mut v, "participants", people.collect());
    texts(
        &mut v,
        "relatedTo",
        entry.task.related_to.keys().cloned().collect(),
    );
    texts(
        &mut v,
        "locations",
        of_map(&json, "locations", "coordinates"),
    );
    texts(&mut v, "links", of_map(&json, "links", "href"));
    let rule = json.get("recurrenceRule").map(Value::to_string);
    texts(&mut v, "recurrenceRules", rule.into_iter().collect());
    let alerts = json.get("alerts").and_then(Value::as_object);
    texts(
        &mut v,
        "alerts",
        alerts
            .into_iter()
            .flat_map(|m| m.values().map(Value::to_string))
            .collect(),
    );
    put(&mut v, "color", strings(&json, "color"));
    put(&mut v, "timeZone", strings(&json, "timeZone"));
    Record {
        id: record_id(entry.id),
        values: v,
    }
}
