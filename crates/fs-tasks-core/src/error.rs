//! Error type of the tasks core.
//!
//! Pattern: none (plain error enum via `thiserror`) — one `Error` for every public operation,
//! with `From` conversions for the store and JSON layers.

use fs_jscalendar::Issue;
use fs_objects::ObjectId;

/// Largest task: its JSON plus the name in its envelope, as an answer carries both and the title counts
/// twice. With the rest of the envelope that always fits a page of the contract
/// (`fs_interfaces::rpc::MAX_INTAKE_BYTES`, half of `MAX_PAGE_BYTES`).
pub const MAX_TASK_BYTES: usize = 16 * 1024 * 1024;

/// Result of every fallible operation of this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong in the tasks core.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The object store failed.
    #[error("object store: {0}")]
    Store(#[from] fs_objects::Error),
    /// The task violates JSCalendar 2.0 (all problems listed).
    #[error("JSCalendar 2.0 violation: {}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))]
    Invalid(Vec<Issue>),
    /// No task with this id.
    #[error("task {0} not found")]
    NotFound(ObjectId),
    /// A date-time, duration or time zone could not be handled.
    #[error("time: {0}")]
    Time(String),
    /// iCalendar text could not be read or written.
    #[error("iCalendar: {0}")]
    ICalendar(String),
    /// JSON could not be read or written.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// The explicit "not possible" of a provider (ADR 0045).
    #[error(transparent)]
    Unsupported(#[from] fs_interfaces::Unsupported),
    /// The task would be too large to come back in one answer (`MAX_TASK_BYTES`).
    #[error("the task and its name have {0} bytes, at most {MAX_TASK_BYTES} are possible")]
    TooLarge(usize),
    /// `edit_checked`: the task was written by someone else since the revision was read. Nothing was
    /// written.
    #[error("conflict: task {0} was changed since it was read; nothing was written")]
    Changed(ObjectId),
    /// The operation conflicts with the current state (duplicate uid, ...).
    #[error("{0}")]
    Conflict(String),
    /// A stored task lacks something this program wrote itself (written by an older version or
    /// by another writer): not the caller's fault.
    #[error("stored task cannot be used: {0}")]
    Stored(String),
}

impl From<fs_jscalendar::Error> for Error {
    fn from(e: fs_jscalendar::Error) -> Self {
        match e {
            fs_jscalendar::Error::Invalid(issues) => Self::Invalid(issues),
            fs_jscalendar::Error::Time(m) => Self::Time(m),
            fs_jscalendar::Error::ICalendar(m) => Self::ICalendar(m),
            fs_jscalendar::Error::Json(e) => Self::Json(e),
        }
    }
}
