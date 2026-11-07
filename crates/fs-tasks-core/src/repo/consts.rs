//! Constants of the task objects.
//!
//! Pattern: none, constants of the task objects: AS2 type, context, media type and index fields.

/// `context` of all objects of this program.
pub const CONTEXT: &str = "tasks";
/// Media type of the object content.
pub const CONTENT_TYPE: &str = "application/jscalendar+json";
/// AS2 type of a task envelope (AS2 has no task type).
pub const TASK_TYPE: &str = "Task";
/// Lowest value of a time index: a time before it stays here, so the text form keeps sorting.
pub(super) const FIRST_INDEX_TIME: &str = "0000-01-01T00:00:00Z";
/// Highest value of a time index, likewise.
pub(super) const LAST_INDEX_TIME: &str = fs_jscalendar::time::FOREVER;
/// Index field with the sort position of the due date: the due time, or [`NO_DUE`] for a task without
/// one, so every task has one and a page can start after any of them.
pub(super) const ORDER_KEY: &str = "order_key";
/// Index field with the object id as text, the last tie-breaker of the order; the store compares
/// ids as text, so does a condition on this field.
pub(super) const ID_KEY: &str = "id_key";
/// Index field with the description, for the search term.
pub(super) const DESCRIPTION: &str = "description";
/// Sorts after every index time: the range is half-open, so a task due at the last second counts. No
/// task can be due at this very text, as date-times refuse second 60.
pub(super) const NO_DUE: &str = "9999-12-31T23:59:60Z";
