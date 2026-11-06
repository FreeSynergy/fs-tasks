//! How to tell a task is marked sensitive (ADR 0029).
//!
//! Pattern: Visitor + Specification — fails closed: a category on the envelope, or the vendor property
//! as a member anywhere in the task (also in a participant or an override), or a JSCalendar `privacy`
//! other than `public` counts as marked. A mere mention in a text does not. **Provisional:**
//! `fs-tasks-core` has no check of its own yet, so this walks the task; replace it with the core's
//! when there is one. The key is the one `fs-people` and `fs-calendar` use.

use fs_tasks_core::TaskEntry;
use serde_json::Value;

/// The vendor property that marks data as sensitive.
const SENSITIVE_KEY: &str = "freesynergy.net:sensitive";

fn value_is_marked(value: &Value) -> bool {
    match value {
        Value::Object(members) => members.iter().any(|(key, member)| {
            let private = key == "privacy" && member.as_str() != Some("public");
            private || key == SENSITIVE_KEY || value_is_marked(member)
        }),
        Value::Array(items) => items.iter().any(value_is_marked),
        _ => false,
    }
}

/// Is this stored task marked sensitive in any form?
pub fn is_marked(entry: &TaskEntry) -> bool {
    !entry.envelope.sensitive.is_empty() || value_is_marked(&entry.task.to_value())
}
