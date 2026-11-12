//! Value types of the task interface.
//!
//! Pattern: Value objects — plain data for results, queries and import reports; `TaskQuery` is a
//! small Builder so callers state only the filters they need.

use chrono::{DateTime, Utc};
use fs_objects::{Envelope, ObjectId};

use crate::model::Task;

/// A stored task with the id it has in the store.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskEntry {
    /// The object id (equal to the `uid` when that is a `urn:uuid:` UUIDv7).
    pub id: ObjectId,
    /// The task.
    pub task: Task,
    /// Its AS2 envelope, as stored (`Task.envelope` in `tasks.capnp`).
    pub envelope: Envelope,
}

/// Filters of [`TaskItems::query`](super::TaskItems::query); all optional, combined with AND.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskQuery {
    /// Due at or after this instant.
    pub due_from: Option<DateTime<Utc>>,
    /// Due before this instant (half-open range, like every range of the interface).
    pub due_to: Option<DateTime<Utc>>,
    /// `progress` value (`needs-action`, `in-process`, `completed`, `failed`, `cancelled`).
    pub progress: Option<String>,
    /// Carries this label (JSCalendar keyword).
    pub label: Option<String>,
    /// Substring of the title or the description, any case (trimmed; empty = no restriction).
    pub term: Option<String>,
    /// Only tasks that are not finished: `progress` `needs-action` or `in-process` (a task without
    /// `progress` counts as `needs-action`).
    pub open: bool,
    /// `Some(true)`: only tasks that are ready to start (nothing they wait for is unfinished; finished tasks are
    /// left out); `Some(false)`: only tasks that are not (blocked, waiting on a cancelled task, unknown, in a cycle,
    /// or finished). Not on the wire yet: `TaskFilter` has no such member.
    pub ready: Option<bool>,
}

impl TaskQuery {
    /// A query without filters (all tasks).
    pub fn new() -> Self {
        Self::default()
    }

    /// Due in `[from, to)`.
    #[must_use]
    pub fn due_between(mut self, from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        self.due_from = Some(from);
        self.due_to = Some(to);
        self
    }

    /// Only tasks with this `progress`.
    #[must_use]
    pub fn with_progress(mut self, progress: impl Into<String>) -> Self {
        self.progress = Some(progress.into());
        self
    }

    /// Only tasks with this label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Only tasks with `term` in the title or the description.
    #[must_use]
    pub fn matching(mut self, term: impl Into<String>) -> Self {
        self.term = Some(term.into());
        self
    }

    /// Only tasks that are ready to start (`true`) or only those that are not (`false`).
    #[must_use]
    pub fn only_ready(mut self, ready: bool) -> Self {
        self.ready = Some(ready);
        self
    }

    /// Only tasks that are not finished.
    #[must_use]
    pub fn only_open(mut self) -> Self {
        self.open = true;
        self
    }
}

/// Result of an import.
#[derive(Clone, Debug, Default)]
pub struct ImportReport {
    /// Tasks that were created.
    pub created: Vec<ObjectId>,
    /// `uid`s that already existed (not overwritten).
    pub skipped: Vec<String>,
    /// Entries that could not be converted or validated.
    pub failed: Vec<String>,
    /// What the 1.0 -> 2.0 upgrade changed.
    pub notes: Vec<String>,
}
