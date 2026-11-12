//! The [`TaskItems`] contract: what the tasks program offers to everyone else.
//!
//! Pattern: Repository (interface) + Null Object — callers (fs-calendar, the GUI, the hub, tests
//! with fakes) depend on this trait, never on the store-backed `TaskRepo`. Like a parent class
//! (ADR 0045) every provider has every method of `Items` in `tasks.capnp` (fs-interfaces): a method
//! it does not override answers an explicit [`Unsupported`], and `capabilities` reports the same.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_interfaces::{Capabilities, Unsupported};
use fs_objects::{ObjectId, StoreId};

use super::page::{Place, TaskPage};
use super::types::{ImportReport, TaskEntry, TaskQuery};
use crate::error::Result;
use crate::model::{ActorType, Relation, Task};
use crate::plan::Readiness;
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
    /// The identity of the store behind this provider, if it has one: made once when the store is
    /// created, never in the log. A provider without a store (a fake) answers `None`.
    fn identity(&self) -> Option<StoreId> {
        None
    }
    /// The task, if it exists.
    async fn get(&self, id: ObjectId) -> Result<Option<TaskEntry>>;
    /// Lookup by the task's `uid`.
    async fn get_by_uid(&self, uid: &str) -> Result<Option<TaskEntry>>;
    /// All tasks, ordered by due date (undated last).
    async fn list(&self) -> Result<Vec<TaskEntry>>;
    /// Tasks matching the filters, ordered by due date (undated last).
    async fn query(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>>;
    /// At most `size` tasks matching `query`, in the order of [`TaskItems::list`], those after `after`
    /// (`None` = from the start). A provider with a store cuts the page in the store; it does not
    /// read the tasks before the place, nor the ones after the page.
    async fn page(&self, query: &TaskQuery, after: Option<&Place>, size: usize)
    -> Result<TaskPage>;

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
    /// `relation.create`: relate the task to the object with `other_uid` (RFC 9253). The task that holds the relation
    /// is the predecessor for the four temporal types (`finishtostart` ...: the other cannot start or finish until this
    /// one does); for `depends-on` it is the dependent one (it depends on `other_uid`).
    async fn relate(
        &self,
        _id: ObjectId,
        _other_uid: &str,
        _relation: Relation,
    ) -> Result<TaskEntry> {
        not_possible("relation.create")
    }
    /// Assign the task to an actor (a person, a program, a service ...) by its AS2 actor IRI, as the owner. Assigning the
    /// same actor twice keeps one participant. Not on the wire yet (`assign` knows only people with an e-mail address).
    async fn assign_actor(
        &self,
        _id: ObjectId,
        _actor: &str,
        _kind: ActorType,
        _name: &str,
    ) -> Result<TaskEntry> {
        not_possible("task.assign")
    }
    /// Take the task for `by` (an actor IRI) until `until`: a claim against being taken twice. Refused with a conflict if
    /// somebody else's claim still holds; the same actor may renew its own. Not on the wire yet.
    async fn claim(&self, _id: ObjectId, _by: &str, _until: DateTime<Utc>) -> Result<TaskEntry> {
        not_possible("task.claim")
    }
    /// Give the claim back: only by the actor that holds it (an expired claim can be released by anyone).
    async fn release_claim(&self, _id: ObjectId, _by: &str) -> Result<TaskEntry> {
        not_possible("task.claim")
    }
    /// `relation.create` with a lag (positive) or lead (negative), an ISO 8601 duration such as `P1D` or `-PT4H` (empty =
    /// none): only for the four temporal relations. A relation that would close a dependency cycle is refused with
    /// [`Error::Cycle`](crate::Error::Cycle) (`dependency.cycle-check`), also by [`TaskItems::relate`].
    async fn relate_with_gap(
        &self,
        _id: ObjectId,
        _other_uid: &str,
        _relation: Relation,
        _gap: &str,
    ) -> Result<TaskEntry> {
        not_possible("relation.create")
    }
    /// `dependency.evaluate`: whether the task can be started, from its dependencies.
    async fn readiness(&self, _id: ObjectId) -> Result<Readiness> {
        not_possible("dependency.evaluate")
    }
    /// `dependency.evaluate` for many tasks at once: one value for each id, in the order asked. The graph around them is
    /// read in batches and the cycles are found once, so the cost does not grow with every task by a full walk
    /// (a store-backed provider; the default asks one by one). Not on the wire yet.
    async fn readiness_many(&self, ids: &[ObjectId]) -> Result<Vec<(ObjectId, Readiness)>> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            out.push((*id, self.readiness(*id).await?));
        }
        Ok(out)
    }
    /// `dependency.evaluate`: the unfinished tasks it waits for (and the cancelled or failed ones, which a person must
    /// decide about).
    async fn blocked_by(&self, _id: ObjectId) -> Result<Vec<TaskEntry>> {
        not_possible("dependency.evaluate")
    }
    /// `dependency.evaluate`: the tasks that wait for it (reverse edges).
    async fn dependents(&self, _id: ObjectId) -> Result<Vec<TaskEntry>> {
        not_possible("dependency.evaluate")
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
