//! How to tell a task is marked sensitive (ADR 0029).
//!
//! Pattern: Visitor + Specification — fails closed: a category on the envelope, the vendor property as a
//! member anywhere in the task, also as a patch path in an override such as
//! `participants/p/freesynergy.net:sensitive`, or a JSCalendar `privacy` other than `public` counts as
//! marked. A mention in a text does not, and neither does a key of an id map: a label called `privacy` is
//! a label. **Provisional:** `fs-tasks-core` has no check of its own yet, so this walks the task; replace
//! it with the core's when there is one. The key is the one `fs-people` and `fs-calendar` use.

use fs_tasks_core::TaskEntry;
use serde_json::Value;

/// The vendor property that marks data as sensitive.
const SENSITIVE_KEY: &str = "freesynergy.net:sensitive";

/// JSCalendar members whose keys are ids or data, not property names. Their values are still walked.
const ID_MAPS: [&str; 18] = [
    "alerts",
    "categories",
    "delegatedFrom",
    "delegatedTo",
    "features",
    "keywords",
    "linkIds",
    "links",
    "localizations",
    "locations",
    "memberOf",
    "participants",
    "recurrenceOverrides",
    "relatedTo",
    "relation",
    "replyTo",
    "roles",
    "virtualLocations",
];

/// One member. A key outside an id map may be a patch path: each part is looked at, and a part after the
/// name of an id map is an id. A key inside an id map is data as a whole, slashes and all.
fn member_is_marked(key: &str, member: &Value, in_map: bool) -> bool {
    if in_map {
        return value_is_marked(member, false);
    }
    let mut next_is_id = false;
    let mut parts = key.split('/').peekable();
    while let Some(part) = parts.next() {
        if next_is_id {
            next_is_id = false;
            continue;
        }
        let is_last = parts.peek().is_none();
        if part == SENSITIVE_KEY
            || (part == "privacy" && (!is_last || member.as_str() != Some("public")))
        {
            return true;
        }
        next_is_id = ID_MAPS.contains(&part);
    }
    value_is_marked(member, next_is_id)
}

fn value_is_marked(value: &Value, in_map: bool) -> bool {
    match value {
        Value::Object(members) => members
            .iter()
            .any(|(key, member)| member_is_marked(key, member, in_map)),
        Value::Array(items) => items.iter().any(|item| value_is_marked(item, false)),
        _ => false,
    }
}

/// Is this stored task marked sensitive in any form?
pub fn is_marked(entry: &TaskEntry) -> bool {
    !entry.envelope.sensitive.is_empty() || value_is_marked(&entry.task.to_value(), false)
}
