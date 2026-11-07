//! `TaskAdapter`: `dyn TaskItems` as a `TaskSource`.
//!
//! Pattern: Adapter over the program's contract (`TaskItems`, in practice the client of `tasks.capnp`),
//! driven with a tokio handle from the interface thread: every call blocks that thread until the
//! server has answered (`DESIGN.md`).
//!
//! Revision: the one the server gives (`RevisedTasks`, `Task.revision`), kept in the draft as text.
//! `save` and `move_to` hand it to `TaskItems::edit_checked`, so the check and the write are one step
//! in the provider; nobody can slip in between. The app cannot derive the revision itself: the wire
//! carries time in milliseconds, the store keeps microseconds.

use std::sync::Arc;

use fs_components_workbenches::SourceError;
use fs_store::ObjectId;
use fs_tasks_client::{Revised, RevisedTasks, TaskPages};
use fs_tasks_core::{Error, Revision, TaskItems};
use tokio::runtime::Handle;

use crate::words::{fill, say};

/// The contract of the tasks program, for the workbench. Do not call it from inside the runtime: it
/// blocks the calling thread until the provider has answered.
pub struct TaskAdapter {
    pub(super) items: Arc<dyn TaskItems>,
    pub(super) revised: Arc<dyn RevisedTasks>,
    pub(super) _pages: Arc<dyn TaskPages>,
    pub(super) runtime: Handle,
}

impl TaskAdapter {
    /// An adapter over a provider of the contract, of the revision and of the pages, driven with
    /// `runtime`.
    pub fn new(
        items: Arc<dyn TaskItems>,
        revised: Arc<dyn RevisedTasks>,
        pages: Arc<dyn TaskPages>,
        runtime: Handle,
    ) -> Self {
        Self {
            items,
            revised,
            _pages: pages,
            runtime,
        }
    }

    /// The task with this key, and its revision.
    pub(super) fn entry(&self, key: &str) -> Result<Revised, SourceError> {
        let id: ObjectId = key.parse().map_err(|_| SourceError::not_found(say::GONE))?;
        self.runtime
            .block_on(self.revised.get_revised(id))
            .map_err(|e| reason(&e))?
            .ok_or_else(|| SourceError::not_found(say::GONE))
    }
}

/// What the core said, in the workbench's terms. A store refusing a sensitive task is said plainly. A
/// conflict of the core (a `uid` taken) is no revision conflict: pressing Save again would not help.
pub(super) fn reason(error: &Error) -> SourceError {
    match error {
        Error::Store(fs_store::Error::EncryptionRequired(_)) => SourceError::failed(fill(
            say::NEEDS_ENCRYPTION,
            &[("error", &error.to_string())],
        )),
        Error::Changed(_) => SourceError::conflict(say::CHANGED_SINCE),
        Error::NotFound(_) => SourceError::not_found(error.to_string()),
        Error::Unsupported(_) => SourceError::unsupported(error.to_string()),
        _ => SourceError::failed(error.to_string()),
    }
}

/// The same reason as plain text.
pub(crate) fn reason_text(error: &Error) -> String {
    reason(error).message
}

/// The revision as the draft holds it: the text of [`Revision`], read back. Without one the task was
/// not read from the server, so there is nothing to compare with: a conflict.
pub(super) fn revision_in(text: Option<&str>) -> Result<Revision, SourceError> {
    text.and_then(|t| u64::from_str_radix(t, 16).ok())
        .map(Revision::from)
        .ok_or_else(|| SourceError::conflict(say::CHANGED_SINCE))
}

/// The revision as text, for a row, a sheet or a draft.
pub(super) fn revision_text(read: &Revised) -> String {
    read.revision.to_string()
}
