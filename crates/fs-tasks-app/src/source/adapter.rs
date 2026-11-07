//! `TaskAdapter`: `dyn TaskItems` as a `TaskSource`.
//!
//! Pattern: Adapter over the program's contract (`TaskItems`, in practice the client of `tasks.capnp`),
//! driven with a tokio handle from the interface thread: every call blocks that thread until the
//! server has answered (`DESIGN.md`).
//!
//! Revision: the store's `updated` time and a fingerprint of the content. `TaskItems::edit` takes no
//! revision, so a write compares and then writes; this is **not atomic** (another writer can slip in
//! between). A compare-and-set is a wish for the core.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use fs_components_workbenches::SourceError;
use fs_store::ObjectId;
use fs_tasks_core::{Error, TaskEntry, TaskItems};
use tokio::runtime::Handle;

use crate::words::say;

/// The contract of the tasks program, for the workbench. Do not call it from inside the runtime: it
/// blocks the calling thread until the provider has answered.
pub struct TaskAdapter {
    pub(super) items: Arc<dyn TaskItems>,
    pub(super) runtime: Handle,
}

impl TaskAdapter {
    /// An adapter over a provider of the contract, driven with `runtime`.
    pub fn new(items: Arc<dyn TaskItems>, runtime: Handle) -> Self {
        Self { items, runtime }
    }

    /// The task with this key.
    pub(super) fn entry(&self, key: &str) -> Result<TaskEntry, SourceError> {
        let id: ObjectId = key
            .parse()
            .map_err(|_| SourceError::not_found("This task no longer exists."))?;
        self.runtime
            .block_on(self.items.get(id))
            .map_err(|e| reason(&e))?
            .ok_or_else(|| SourceError::not_found("This task no longer exists."))
    }

    /// The task with this key, if it is still as it was at `revision`.
    pub(super) fn entry_at(
        &self,
        key: &str,
        revision: Option<&str>,
    ) -> Result<TaskEntry, SourceError> {
        let entry = self.entry(key)?;
        if revision != Some(revision_of(&entry).as_str()) {
            return Err(SourceError::conflict(say::CHANGED_SINCE));
        }
        Ok(entry)
    }
}

/// What the core said, in the workbench's terms. A store refusing a sensitive task is said plainly. A
/// conflict of the core (a `uid` taken) is no revision conflict: pressing Save again would not help.
pub(super) fn reason(error: &Error) -> SourceError {
    match error {
        Error::Store(fs_store::Error::EncryptionRequired(_)) => SourceError::failed(format!(
            "this task is marked sensitive and this store is not encrypted, so it was refused ({error})"
        )),
        Error::NotFound(_) => SourceError::not_found(error.to_string()),
        Error::Unsupported(_) => SourceError::unsupported(error.to_string()),
        _ => SourceError::failed(error.to_string()),
    }
}

/// The same reason as plain text.
pub(crate) fn reason_text(error: &Error) -> String {
    reason(error).message
}

/// `updated` alone could repeat within the store's clock tick.
pub(super) fn revision_of(entry: &TaskEntry) -> String {
    let mut hash = DefaultHasher::new();
    entry.task.to_value().to_string().hash(&mut hash);
    format!(
        "{}/{:x}",
        entry.envelope.updated.to_rfc3339(),
        hash.finish()
    )
}
