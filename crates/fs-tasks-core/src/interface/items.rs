//! The [`TaskItems`] contract: what the tasks program offers to everyone else.
//!
//! Pattern: Repository (interface) + Null Object — callers (fs-calendar, the GUI, the hub, tests
//! with fakes) depend on this trait, never on the store-backed `TaskRepo`. Like a parent class
//! (ADR 0045) every provider has every method of `Items` in `tasks.capnp` (fs-interfaces): a method
//! it does not override answers an explicit [`Unsupported`], and `capabilities` reports the same.

use async_trait::async_trait;
use fs_interfaces::{Capabilities, Unsupported};
use fs_objects::ObjectId;

use super::types::{ImportReport, TaskEntry, TaskQuery};
use crate::error::Result;
use crate::model::{Relation, Task};
use crate::revision::Revision;

/// Capability ids written in `tasks.capnp` (ADR 0021): the five that name a write.
pub const CAPABILITIES: [&str; 5] = [
    "task.create",
    "task.edit",
    "task.assign",
    "relation.create",
    "label.assign",
];

/// The explicit "not possible" of a method a provider does not override.
pub(crate) fn not_possible<T>(capability: &str) -> Result<T> {
    Err(Unsupported::new(capability, "this provider does not offer it").into())
}

/// Tasks: create, edit, assign, relate, label and read them.
///
/// Interface status: draft (ADR 0045). Every method may still change until two independent
/// consumers have tested it.
#[async_trait]
pub trait TaskItems: Send + Sync {
    /// What this provider can do, per capability id.
    fn capabilities(&self) -> Capabilities;
    /// The task, if it exists.
    async fn get(&self, id: ObjectId) -> Result<Option<TaskEntry>>;
    /// Lookup by the task's `uid`.
    async fn get_by_uid(&self, uid: &str) -> Result<Option<TaskEntry>>;
    /// All tasks, ordered by due date (undated last).
    async fn list(&self) -> Result<Vec<TaskEntry>>;
    /// Tasks matching the filters, ordered by due date (undated last).
    async fn query(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>>;

    /// `task.create`: validate and store a new task. Refused if its `uid` already exists.
    async fn create(&self, _task: Task) -> Result<TaskEntry> {
        not_possible("task.create")
    }
    /// `task.edit`: replace the task (`uid` cannot change; `sequence` follows the change).
    async fn edit(&self, _id: ObjectId, _task: Task) -> Result<TaskEntry> {
        not_possible("task.edit")
    }
    /// `task.edit`, compare-and-set: replace the task only if it still has `expected` (the
    /// [`Revision`] the caller read); otherwise [`Error::Changed`](crate::Error::Changed) and nothing is
    /// written. The check and the write are one step, atomic inside one provider process only.
    async fn edit_checked(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _task: Task,
    ) -> Result<TaskEntry> {
        not_possible("task.edit")
    }
    /// `task.assign`: make `email` an owner of the task.
    async fn assign(&self, _id: ObjectId, _name: &str, _email: &str) -> Result<TaskEntry> {
        not_possible("task.assign")
    }
    /// `relation.create`: relate the task to the object with `other_uid` (RFC 9253).
    async fn relate(
        &self,
        _id: ObjectId,
        _other_uid: &str,
        _relation: Relation,
    ) -> Result<TaskEntry> {
        not_possible("relation.create")
    }
    /// `label.assign`: attach a label.
    async fn label(&self, _id: ObjectId, _label: &str) -> Result<TaskEntry> {
        not_possible("label.assign")
    }
    /// `task.delete`: delete a task (undoable through the store).
    async fn delete(&self, _id: ObjectId) -> Result<()> {
        not_possible("task.delete")
    }
    /// `task.import`: import the `VTODO`s of an `.ics` text; known `uid`s are skipped.
    async fn import_ics(&self, _text: &str) -> Result<ImportReport> {
        not_possible("task.import")
    }
    /// `task.export`: the tasks as iCalendar text.
    async fn export_ics(&self, _ids: &[ObjectId]) -> Result<String> {
        not_possible("task.export")
    }
}
