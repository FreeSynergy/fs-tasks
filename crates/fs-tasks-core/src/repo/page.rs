//! A page of tasks, cut in the store at a place in the sort key.
//!
//! Pattern: Iterator (external, resumable) over the Repository — the page after a [`Place`] is
//! asked of the store as at most three queries, each with a limit, never as the whole list that is
//! cut afterwards. The order is the store's `order_by_index` of `order_key`: the smallest case-folded
//! value, then the case-folded title (`name_sort`), then the id as text. The conditions compare
//! the same three things the same way (`Gt` on the raw `order_key`, which is one fixed shape of
//! text, so raw and folded order agree; the name through the store's own folding; the id as text),
//! so no task falls between two pages and none comes twice.

use fs_objects::{Cond, Op, Query, StoredObject};

use super::consts::{CONTEXT, TASK_TYPE};

use super::consts::{ID_KEY, ORDER_KEY};
use super::crud::entry_of;
use super::handle::TaskRepo;
use super::queries::base;
use crate::error::{Error, Result};
use crate::interface::{Place, Placed, TaskPage, TaskQuery};

/// The queries that together give everything after `place`, in order: the rest of the same due
/// date and title (by id), the later titles of the same due date, the later due dates.
fn after(base: Query, place: &Place) -> Vec<Query> {
    let same_due = |q: Query| q.with_index(ORDER_KEY, &place.order);
    let name = |op| Cond::Name {
        op,
        value: place.name.clone(),
    };
    vec![
        same_due(base.clone())
            .and(name(Op::Eq))
            .index_op(ID_KEY, Op::Gt, place.id.to_string()),
        same_due(base.clone()).and(name(Op::Gt)),
        base.index_op(ORDER_KEY, Op::Gt, &place.order),
    ]
}

/// Where an object stands in the order: its order key, name and id. A task without an order key (written before 0.7.0)
/// has none.
pub(super) fn place_of(object: &StoredObject) -> Result<Place> {
    let id = object.envelope.id;
    let order = object
        .index
        .get(ORDER_KEY)
        .and_then(|values| values.first())
        .cloned()
        .ok_or_else(|| Error::Stored(format!("task {id} has no {ORDER_KEY}; write it again")))?;
    Ok(Place {
        order,
        name: object.envelope.name.clone(),
        id,
    })
}

pub(super) fn placed(object: StoredObject) -> Result<Placed> {
    let place = place_of(&object)?;
    Ok(Placed {
        entry: entry_of(object)?,
        place,
    })
}

impl TaskRepo {
    /// The first page checks that every task has a place in the order. One that has not (written
    /// before 0.7.0, or not repairable) would be sorted last by the store and then be skipped by every
    /// later page; so it is said here, with the way out, and nothing is cut short unnoticed.
    pub(super) async fn check_places(&self) -> Result<()> {
        let all = Query::new().of_type(TASK_TYPE).in_context(CONTEXT);
        let placed = all.clone().index_op(ORDER_KEY, Op::Ge, "");
        let (all, placed) = (
            self.queries.count(&all).await?,
            self.queries.count(&placed).await?,
        );
        if placed < all {
            return Err(Error::Stored(format!(
                "{} of {all} tasks have no place in the order (written before 0.7.0 or not repairable); run upgrade_index and read its report",
                all - placed
            )));
        }
        Ok(())
    }

    pub(super) async fn page_inner(
        &self,
        query: &TaskQuery,
        place: Option<&Place>,
        size: usize,
    ) -> Result<TaskPage> {
        if place.is_none() {
            self.check_places().await?;
        }
        // One more than asked: that is how a page knows there is a next one.
        let want = size.saturating_add(1);
        let base = base(query);
        let steps = match place {
            Some(p) => after(base, p),
            None => vec![base],
        };
        let mut found: Vec<StoredObject> = Vec::new();
        for step in steps {
            let missing = want - found.len();
            if missing == 0 {
                break;
            }
            found.extend(self.queries.query(&step.limit(missing)).await?);
        }
        let more = found.len() > size;
        found.truncate(size);
        Ok(TaskPage {
            items: found.into_iter().map(placed).collect::<Result<_>>()?,
            more,
        })
    }
}
