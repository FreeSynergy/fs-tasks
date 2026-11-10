//! The identity of a record: `urn:uuid:<object id>`.
//!
//! Pattern: Adapter — the front end wants an absolute IRI (an AS2 `id`) for a record; the store has an `ObjectId`.
//! One place turns one into the other, so no other file knows the prefix.

use fs_objects::ObjectId;

const PREFIX: &str = "urn:uuid:";

/// The record id of a stored task.
pub fn record_id(id: ObjectId) -> String {
    format!("{PREFIX}{id}")
}

/// The object id of a record id (`None` for a text that is not one).
pub fn object_id(record: &str) -> Option<ObjectId> {
    record.trim().strip_prefix(PREFIX)?.parse().ok()
}
