//! Constants of the task objects.
//!
//! Pattern: Registry (constants) — the AS2 type, context and media type used by this program in one place.

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
