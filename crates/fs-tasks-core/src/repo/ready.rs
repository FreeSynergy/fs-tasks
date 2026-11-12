//! The `ready` filter: queries and pages limited to the tasks that can be started (or those that cannot).
//!
//! Pattern: Decorator over the store query — the store query runs without the filter and returns the matches in the
//! store's order; they are judged once against the graph of the store (read by one scan), and the page is cut from the
//! ones that match, so the place a page ends at is still a place in the store's order. The cost is one query, one scan
//! and one linear judgement, however many tasks the page has to skip.

use std::collections::{BTreeMap, BTreeSet};

use fs_objects::StoredObject;

use super::consts::ORDER_KEY;
use super::crud::entry_of;
use super::deps::{DEPENDS_ON, PRECEDES};
use super::handle::TaskRepo;
use super::page;
use super::queries::base;
use crate::error::Result;
use crate::interface::{Place, Placed, TaskEntry, TaskPage, TaskQuery};
use crate::model::Task;
use crate::plan::{Readiness, edges_of, readiness_of};

/// The query as the store sees it: no `ready`, and only open tasks when ready ones are wanted (a finished task is not).
fn inner_query(query: &TaskQuery, wanted: bool) -> TaskQuery {
    let mut inner = query.clone();
    inner.ready = None;
    if wanted {
        inner.open = true;
    }
    inner
}

/// The sort key of the scan path: the order key of the index, the lower-case name, the id as text.
fn sort_key(o: &StoredObject) -> (String, String, String) {
    (
        o.index
            .get(ORDER_KEY)
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_default(),
        o.envelope.name.to_lowercase(),
        o.envelope.id.to_string(),
    )
}

/// Does the query hold nothing but `open` and `ready`? Then one scan answers it.
fn only_ready_and_open(q: &TaskQuery) -> bool {
    q.due_from.is_none()
        && q.due_to.is_none()
        && q.progress.is_none()
        && q.label.is_none()
        && q.term.as_deref().is_none_or(|t| t.trim().is_empty())
}

impl TaskRepo {
    /// The readiness of `entries` against the whole graph of the store, which was read once for the whole question
    /// (`graph`): each chunk of a filtered query is judged in memory, not by new queries.
    fn judge_in(
        graph: &BTreeMap<String, Task>,
        entries: &[TaskEntry],
    ) -> BTreeMap<String, Readiness> {
        let mut all: BTreeMap<&str, &Task> = graph.iter().map(|(u, t)| (u.as_str(), t)).collect();
        for e in entries {
            all.insert(e.task.uid.as_str(), &e.task);
        }
        let tasks: Vec<Task> = all.values().map(|t| (*t).clone()).collect();
        let mut judged = readiness_of(&tasks);
        let wanted: BTreeSet<&str> = entries.iter().map(|e| e.task.uid.as_str()).collect();
        judged.retain(|uid, _| wanted.contains(uid.as_str()));
        judged
    }

    /// The matches of a query that has no filter but `open` and `ready`, found with one scan of the store: the objects that
    /// match, in the order `(order key, lower-case name, id)`. Only the tasks that hold a dependency (and those they
    /// name) are parsed to be judged; any other task is ready exactly when it is not finished, which its index says.
    async fn ready_by_scan(&self, query: &TaskQuery, wanted: bool) -> Result<Vec<StoredObject>> {
        let objects = self.scan_objects().await?;
        let mut parsed: BTreeMap<String, Task> = BTreeMap::new();
        for o in &objects {
            if o.index.contains_key(PRECEDES) || o.index.contains_key(DEPENDS_ON) {
                let task = entry_of(o.clone())?.task;
                parsed.insert(task.uid.clone(), task);
            }
        }
        let named: BTreeSet<String> = parsed
            .values()
            .flat_map(|t| {
                edges_of(std::slice::from_ref(t))
                    .into_iter()
                    .flat_map(|e| [e.from, e.to])
            })
            .collect();
        for o in &objects {
            let uid = o.index.get("uid").and_then(|v| v.first());
            if let Some(uid) = uid.filter(|u| named.contains(*u) && !parsed.contains_key(*u)) {
                let task = entry_of(o.clone())?.task;
                parsed.insert(uid.clone(), task);
            }
        }
        if parsed.len() > self.max_loaded {
            return Err(self.too_big());
        }
        let tasks: Vec<Task> = parsed.values().cloned().collect();
        let judged = readiness_of(&tasks);
        let mut out: Vec<StoredObject> = objects
            .into_iter()
            .filter(|o| {
                let progress = o
                    .index
                    .get("progress")
                    .and_then(|v| v.first())
                    .map_or("needs-action", String::as_str);
                if query.open && !matches!(progress, "needs-action" | "in-process") {
                    return false;
                }
                let uid = o.index.get("uid").and_then(|v| v.first());
                let ready = match uid.and_then(|u| judged.get(u)) {
                    Some(r) => *r == Readiness::Ready,
                    None => matches!(progress, "needs-action" | "in-process"),
                };
                ready == wanted
            })
            .collect();
        out.sort_by_cached_key(sort_key);
        Ok(out)
    }

    /// The tasks of a query with `ready` set, in the order of the store, as objects: one store query for the matches, one
    /// scan for the graph, one judgement of all of them.
    async fn ready_objects(&self, query: &TaskQuery, wanted: bool) -> Result<Vec<StoredObject>> {
        if only_ready_and_open(query) {
            return self.ready_by_scan(query, wanted).await;
        }
        let inner = inner_query(query, wanted);
        let found = self.queries.query(&base(&inner)).await?;
        let graph = self.scan_graph().await?;
        let entries: Vec<TaskEntry> = found.iter().cloned().map(entry_of).collect::<Result<_>>()?;
        let judged = Self::judge_in(&graph, &entries);
        Ok(found
            .into_iter()
            .zip(entries)
            .filter(|(_, e)| {
                judged
                    .get(&e.task.uid)
                    .is_some_and(|r| (*r == Readiness::Ready) == wanted)
            })
            .map(|(o, _)| o)
            .collect())
    }

    /// The tasks of a query with `ready` set.
    pub(super) async fn query_ready(
        &self,
        query: &TaskQuery,
        wanted: bool,
    ) -> Result<Vec<TaskEntry>> {
        self.ready_objects(query, wanted)
            .await?
            .into_iter()
            .map(entry_of)
            .collect()
    }

    /// A page of a query with `ready` set: the matches are found and judged once, in the store's order, and the page is
    /// cut after the place (a place whose task is gone or changed is found by the sort key).
    pub(super) async fn page_ready(
        &self,
        query: &TaskQuery,
        wanted: bool,
        place: Option<&Place>,
        size: usize,
    ) -> Result<TaskPage> {
        if place.is_none() && !only_ready_and_open(query) {
            self.check_places().await?;
        }
        let objects = self.ready_objects(query, wanted).await?;
        let mut placed: Vec<(Place, StoredObject)> = objects
            .into_iter()
            .map(|o| Ok((page::place_of(&o)?, o)))
            .collect::<Result<_>>()?;
        if let Some(p) = place {
            let key = |p: &Place| (p.order.clone(), p.name.to_lowercase(), p.id.to_string());
            match placed.iter().position(|(x, _)| x.id == p.id) {
                Some(at) => {
                    placed.drain(..=at);
                }
                None => placed.retain(|(x, _)| key(x) > key(p)),
            }
        }
        let more = placed.len() > size;
        placed.truncate(size);
        // only the tasks of the page are parsed
        let items: Vec<Placed> = placed
            .into_iter()
            .map(|(place, o)| {
                Ok(Placed {
                    entry: entry_of(o)?,
                    place,
                })
            })
            .collect::<Result<_>>()?;
        Ok(TaskPage { items, more })
    }
}
