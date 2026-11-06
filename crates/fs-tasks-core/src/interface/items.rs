//! The [`TaskItems`] contract: what the tasks program offers to everyone else.
//!
//! Pattern: Repository (interface) — callers (fs-calendar, the GUI, the hub, tests with fakes)
//! depend on this trait, never on the store-backed `TaskRepo`. Mirrors the `Items` interface of
//! `schema/tasks.capnp`; capability ids follow `design.md`.

use async_trait::async_trait;
use fs_store::ObjectId;

use super::types::{ImportReport, TaskEntry, TaskQuery};
use crate::error::Result;
use crate::model::{Relation, Task};

/// Capability ids of the `Items` interface (ADR 0021).
pub const CAPABILITIES: [&str; 5] = [
    "task.create",
    "task.edit",
    "task.assign",
    "relation.create",
    "label.assign",
];

/// Tasks: create, edit, assign, relate, label and read them.
///
/// Interface status: draft (ADR 0045). Every method may still change until two independent
/// consumers have tested it.
#[async_trait]
pub trait TaskItems: Send + Sync {
    /// The capability ids this implementation offers.
    fn capabilities(&self) -> Vec<String> {
        CAPABILITIES.iter().map(ToString::to_string).collect()
    }
    /// `task.create`: validate and store a new task. Refused if its `uid` already exists.
    async fn create(&self, task: Task) -> Result<TaskEntry>;
    /// `task.edit`: replace the task (`uid` cannot change; `sequence` follows the change).
    async fn edit(&self, id: ObjectId, task: Task) -> Result<TaskEntry>;
    /// `task.assign`: make `email` an owner of the task.
    async fn assign(&self, id: ObjectId, name: &str, email: &str) -> Result<TaskEntry>;
    /// `relation.create`: relate the task to the object with `other_uid` (RFC 9253).
    async fn relate(&self, id: ObjectId, other_uid: &str, relation: Relation) -> Result<TaskEntry>;
    /// `label.assign`: attach a label.
    async fn label(&self, id: ObjectId, label: &str) -> Result<TaskEntry>;
    /// The task, if it exists.
    async fn get(&self, id: ObjectId) -> Result<Option<TaskEntry>>;
    /// Lookup by the task's `uid`.
    async fn get_by_uid(&self, uid: &str) -> Result<Option<TaskEntry>>;
    /// Delete a task (undoable through the store).
    async fn delete(&self, id: ObjectId) -> Result<()>;
    /// All tasks, ordered by due date (undated last).
    async fn list(&self) -> Result<Vec<TaskEntry>>;
    /// Tasks matching the filters, ordered by due date (undated last).
    async fn query(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>>;
    /// Import the `VTODO`s of an `.ics` text; known `uid`s are skipped.
    async fn import_ics(&self, text: &str) -> Result<ImportReport>;
    /// The tasks as iCalendar text.
    async fn export_ics(&self, ids: &[ObjectId]) -> Result<String>;
}
