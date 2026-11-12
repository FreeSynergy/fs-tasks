//! Listing and filtered queries.
//!
//! Pattern: Repository (read side) + Builder consumer — a [`TaskQuery`] is translated into one
//! store query; the store filters and orders, nothing is selected in memory. The same translation
//! serves the pages (`page.rs`).

use chrono::{DateTime, Datelike, Duration, Utc};
use fs_objects::{Cond, Op, Query};

use super::consts::{CONTEXT, DESCRIPTION, FIRST_INDEX_TIME, NO_DUE, ORDER_KEY, TASK_TYPE};
use super::crud::entry_of;
use super::handle::TaskRepo;
use crate::error::Result;
use crate::interface::{TaskEntry, TaskQuery};
use crate::time::fmt_utc;

/// A bound of a half-open range. The index holds whole seconds, so a bound with a fraction rounds up:
/// a task due at second `s` is at or after `from` exactly if `s >= ceil(from)`, likewise before `to`.
/// Outside the years 0 to 9999 the text form no longer sorts, so the bound stops there.
fn bound(t: DateTime<Utc>) -> String {
    let ceil = if t.timestamp_subsec_nanos() > 0 {
        t.checked_add_signed(Duration::seconds(1))
    } else {
        Some(t)
    };
    match ceil {
        Some(c) if (0..10_000).contains(&c.year()) => fmt_utc(c),
        Some(c) if c.year() < 0 => FIRST_INDEX_TIME.into(),
        _ => NO_DUE.into(),
    }
}

/// The first value of `progress` that is open: the values are a closed list (`validate`), and by
/// text `cancelled < completed < failed < in-process < needs-action`, so open is "from here on".
const FIRST_OPEN: &str = "in-process";

/// The store query for `query`, in the order of the list: due date (undated last), title, id. A
/// task without a due date has the sort position `NO_DUE`, so a place in this order always exists.
pub(super) fn base(query: &TaskQuery) -> Query {
    let mut q = Query::new()
        .of_type(TASK_TYPE)
        .in_context(CONTEXT)
        .order_by_index(ORDER_KEY);
    if query.due_from.is_some() || query.due_to.is_some() {
        let from = query
            .due_from
            .map_or_else(|| FIRST_INDEX_TIME.into(), bound);
        let to = query.due_to.map_or_else(|| NO_DUE.into(), bound);
        q = q.index_between("due_utc", from, to);
    }
    if let Some(p) = &query.progress {
        q = q.with_index("progress", p);
    }
    if let Some(l) = &query.label {
        q = q.with_index("label", l);
    }
    if let Some(actor) = &query.assignee_actor {
        q = q.with_index("assignee_actor", actor);
    }
    if query.open {
        q = q.index_op("progress", Op::Ge, FIRST_OPEN);
    }
    match query.term.as_deref().map(str::trim) {
        Some(term) if !term.is_empty() => q
            .or(Cond::Name {
                op: Op::Contains,
                value: term.into(),
            })
            .or(Cond::Index {
                key: DESCRIPTION.into(),
                op: Op::Contains,
                value: term.into(),
            }),
        _ => q,
    }
}

impl TaskRepo {
    pub(super) async fn query_inner(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>> {
        self.queries
            .query(&base(query))
            .await?
            .into_iter()
            .map(entry_of)
            .collect()
    }
}
