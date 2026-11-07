//! `RevisedTasks`: a task together with the revision the server gives it.
//!
//! Pattern: Strategy (a narrow trait beside `TaskItems`, so a frontend depends on `dyn RevisedTasks`
//! and not on [`RpcTasks`](crate::RpcTasks)). Why a trait of its own: `TaskEntry` has no place for the
//! revision, and it cannot be derived from the entry read here (the wire keeps time in milliseconds,
//! the store in microseconds), so the number from `Task.revision` is handed on as it came. It is for
//! [`TaskItems::edit_checked`](fs_tasks_core::TaskItems::edit_checked); compare it for equality only.
//! It lives in the client for now and moves up to `fs-tasks-core` with a second user (`DESIGN.md`).

use async_trait::async_trait;
use fs_store::ObjectId;
use fs_tasks_core::{Result, Revision, TaskEntry};

/// A task as read, and its revision.
#[derive(Debug, Clone)]
pub struct Revised {
    /// The task.
    pub entry: TaskEntry,
    /// What the server says the task is now: the `expected` of `edit_checked`.
    pub revision: Revision,
}

/// Reading tasks with their revisions.
#[async_trait]
pub trait RevisedTasks: Send + Sync {
    /// The task and its revision; `None` when there is no such task.
    async fn get_revised(&self, id: ObjectId) -> Result<Option<Revised>>;

    /// Every task with its revision, in the order of `TaskItems::list`.
    async fn list_revised(&self) -> Result<Vec<Revised>>;
}
