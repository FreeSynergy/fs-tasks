//! Constants of the task objects.
//!
//! Pattern: Registry (constants) — the AS2 type, context and media type used by this program in one place.

/// `context` of all objects of this program.
pub const CONTEXT: &str = "tasks";
/// Media type of the object content.
pub const CONTENT_TYPE: &str = "application/jscalendar+json";
/// AS2 type of a task envelope (AS2 has no task type).
pub const TASK_TYPE: &str = "Task";
