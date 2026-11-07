//! `TaskPages`: the tasks of a query, one page at a time, continued by a cursor.
//!
//! Pattern: Iterator, external and resumable — a page is one step, the [`Cursor`] is the state that
//! says where the next one starts. The steps sit behind a trait (Strategy), so a frontend depends on
//! `dyn TaskPages` and not on [`RpcTasks`](crate::RpcTasks); [`PageWalk`](crate::PageWalk) (`walk.rs`)
//! is the iterator over it. `TaskItems` stays the Repository: this trait is a second, narrower way
//! to read the same list (`list` and `query` of `tasks.capnp` with their `cursor`). Like
//! [`RevisedTasks`](crate::RevisedTasks) it lives in the client for now and moves up to
//! `fs-tasks-core` with a second user (`DESIGN.md`).
//!
//! The total is not known: a page says only whether another follows.

use async_trait::async_trait;
use fs_tasks_core::{Result, TaskQuery};

use crate::revised::Revised;

/// Where the next page starts: opaque, handed back as it came. A made-up value is refused by the
/// server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor(Vec<u8>);

impl Cursor {
    /// A cursor from bytes kept elsewhere (or made up, to see the refusal).
    #[must_use]
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The bytes, as the wire carries them.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// One page of tasks, with their revisions, in the order of the contract (by due date, undated last).
#[derive(Debug, Clone)]
pub struct TaskPage {
    /// The tasks of this page.
    pub items: Vec<Revised>,
    /// Where the next page starts; `None` when this was the last.
    pub next: Option<Cursor>,
}

/// Page-wise reading of the task list. The server decides the size of a page.
#[async_trait]
pub trait TaskPages: Send + Sync {
    /// The first page of what `query` selects (an empty query selects every task).
    ///
    /// # Errors
    /// The provider could not be reached or refused the query.
    async fn first(&self, query: &TaskQuery) -> Result<TaskPage>;

    /// The page that starts at `cursor`, for the same `query` the cursor came from.
    ///
    /// # Errors
    /// The cursor is not one the server handed out (`Error::Store(Invalid)`), or as for `first`.
    async fn next(&self, query: &TaskQuery, cursor: &Cursor) -> Result<TaskPage>;
}
