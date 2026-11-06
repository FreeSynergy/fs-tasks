//! Error type of the tasks core.
//!
//! Pattern: none (plain error enum via `thiserror`) — one `Error` for every public operation,
//! with `From` conversions for the store and JSON layers.

use fs_jscalendar::Issue;
use fs_store::ObjectId;

/// Result of every fallible operation of this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong in the tasks core.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The object store failed.
    #[error("object store: {0}")]
    Store(#[from] fs_store::Error),
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
    /// The operation conflicts with the current state (duplicate uid, ...).
    #[error("{0}")]
    Conflict(String),
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
