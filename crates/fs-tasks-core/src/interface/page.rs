//! A page of tasks and the place it ends at.
//!
//! Pattern: Value object — plain data of the paging (`Place`, `Placed`, `TaskPage`). A page is read
//! from a place in the sort key (due date, title, id), not from a count, so a task added or
//! removed between two pages moves no boundary.

use fs_store::ObjectId;

use super::types::TaskEntry;

/// A place in the order of the tasks: the three parts of the sort key of the store, of the task
/// after which the next page starts. The parts are as the store keeps them; only the repository
/// that wrote them reads them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    /// The sort position of the due date, `order_key` of the index (undated sorts last).
    pub order: String,
    /// The title as the envelope has it; the store compares it case-folded.
    pub name: String,
    /// The object id (the last tie-breaker).
    pub id: ObjectId,
}

/// A task and its place.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// The task.
    pub entry: TaskEntry,
    /// Where it stands in the order.
    pub place: Place,
}

/// One page of tasks in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaskPage {
    /// At most the size asked for.
    pub items: Vec<Placed>,
    /// There is at least one more task after the last of `items`.
    pub more: bool,
}
