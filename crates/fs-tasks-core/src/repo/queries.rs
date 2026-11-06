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
use crate::time::{FOREVER, fmt_utc};

/// Lowest `UTCDateTime`: lower bound of a range without `due_from`.
const EPOCH: &str = "0000-01-01T00:00:00Z";

/// A bound of a half-open range. The index holds whole seconds, so a bound with a fraction rounds up:
/// a task due at second `s` is at or after `from` exactly if `s >= ceil(from)`, likewise before `to`.
/// Outside the years 0 to 9999 the text form no longer sorts, so the bound stops there.
fn bound(t: DateTime<Utc>) -> String {
    let ceil = if t.timestamp_subsec_nanos() > 0 {
        t + Duration::seconds(1)
    } else {
        t
    };
    match ceil.year() {
        ..0 => EPOCH.into(),
        10_000.. => FOREVER.into(),
        _ => fmt_utc(ceil),
    }
}

impl TaskRepo {
    pub(super) async fn query_inner(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>> {
        let mut q = Query::new().of_type(TASK_TYPE).in_context(CONTEXT);
        q = if query.due_from.is_some() || query.due_to.is_some() {
            let from = query.due_from.map_or_else(|| EPOCH.into(), bound);
            let to = query.due_to.map_or_else(|| FOREVER.into(), bound);
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
