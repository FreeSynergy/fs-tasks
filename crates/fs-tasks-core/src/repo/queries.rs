//! Listing and filtered queries.
//!
//! Pattern: Repository (read side) + Builder consumer — a [`TaskQuery`] is translated into one
//! store query; the store filters and orders, nothing is selected in memory.

use chrono::{DateTime, Datelike, Duration, Utc};
use fs_store::Query;

use super::consts::{CONTEXT, TASK_TYPE};
use super::crud::entry_of;
use super::handle::TaskRepo;
use crate::error::Result;
use crate::interface::{TaskEntry, TaskQuery};
use crate::time::fmt_utc;

/// Sorts after every `UTCDateTime`: the range is half-open, so a task due at the last second counts.
const OPEN_END: &str = "9999-12-31T23:59:60Z";
/// Lowest `UTCDateTime`: lower bound of a range without `due_from`.
const EPOCH: &str = "0000-01-01T00:00:00Z";

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
        Some(c) if c.year() < 0 => EPOCH.into(),
        _ => OPEN_END.into(),
    }
}

impl TaskRepo {
    pub(super) async fn query_inner(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>> {
        let mut q = Query::new().of_type(TASK_TYPE).in_context(CONTEXT);
        q = if query.due_from.is_some() || query.due_to.is_some() {
            let from = query.due_from.map_or_else(|| EPOCH.into(), bound);
            let to = query.due_to.map_or_else(|| OPEN_END.into(), bound);
            q.index_between("due_utc", from, to)
                .order_by_index("due_utc")
        } else {
            // by due date (undated last), by the store; the same order as a range
            q.order_by_index("due_utc")
        };
        if let Some(p) = &query.progress {
            q = q.with_index("progress", p);
        }
        if let Some(l) = &query.label {
            q = q.with_index("label", l);
        }
        self.queries
            .query(&q)
            .await?
            .into_iter()
            .map(entry_of)
            .collect()
    }
}
