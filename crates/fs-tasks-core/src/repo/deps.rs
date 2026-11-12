//! Dependencies in the store: readiness, the neighbours of a task, the cycle check of `relate`, the `ready` filter.
//!
//! Pattern: Facade over the pure functions of `plan` — this file only reads the tasks the question needs (the task, its
//! predecessors, what is reachable behind it) through the index fields `precedes` and `depends_on`, and hands them to
//! `plan`. The index is what makes a reverse lookup ("who waits for this task?") a query and not a scan.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use fs_objects::{ObjectId, Query};

use super::consts::{CONTEXT, TASK_TYPE};
use super::crud::entry_of;
use super::handle::TaskRepo;
use crate::error::{Error, Result};
use crate::interface::{Place, Placed, TaskEntry, TaskPage, TaskQuery};
use crate::model::{Relation, Task};
use crate::plan::{Edge, Readiness, edges_of, path_closing_cycle, readiness_of};

/// Index field: the `uid`s of the successors named by the temporal relations a task holds.
pub(super) const PRECEDES: &str = "precedes";
/// Index field: the `uid`s a task depends on (`depends-on`).
pub(super) const DEPENDS_ON: &str = "depends_on";
/// Index field present on every task whose dependency fields are written (older tasks lack it; `upgrade_index`).
pub(super) const REL_INDEX: &str = "rel_index";

/// Most tasks one question about the graph reads; more is refused, so a huge or hostile graph cannot hold the store.
const MAX_GRAPH: usize = 5000;
/// Tasks read per round when the `ready` filter fills a page.
const CHUNK: usize = 50;

impl TaskRepo {
    async fn tasks_with(&self, field: &str, uid: &str) -> Result<Vec<TaskEntry>> {
        let q = Query::new()
            .of_type(TASK_TYPE)
            .in_context(CONTEXT)
            .with_index(field, uid);
        self.queries
            .query(&q)
            .await?
            .into_iter()
            .map(entry_of)
            .collect()
    }

    async fn entry_by_uid(&self, uid: &str) -> Result<Option<TaskEntry>> {
        self.stored_by_uid(uid).await?.map(entry_of).transpose()
    }

    /// The tasks that wait for `task` (its successors): the targets of the temporal relations it holds, and the tasks
    /// that depend on it.
    pub(super) async fn successors(&self, task: &Task) -> Result<Vec<TaskEntry>> {
        let mut found: BTreeMap<String, TaskEntry> = BTreeMap::new();
        for edge in edges_of(std::slice::from_ref(task))
            .into_iter()
            .filter(|e| e.from == task.uid)
        {
            if let Some(entry) = self.entry_by_uid(&edge.to).await? {
                found.insert(edge.to, entry);
            }
        }
        for entry in self.tasks_with(DEPENDS_ON, &task.uid).await? {
            found.insert(entry.task.uid.clone(), entry);
        }
        Ok(found.into_values().collect())
    }

    /// The tasks `task` waits for (its predecessors): what it depends on, and the tasks that hold a temporal relation
    /// to it. Predecessors that are not in this store are not here (readiness names them as unknown).
    pub(super) async fn predecessors(&self, task: &Task) -> Result<Vec<TaskEntry>> {
        let mut found: BTreeMap<String, TaskEntry> = BTreeMap::new();
        for edge in edges_of(std::slice::from_ref(task))
            .into_iter()
            .filter(|e| e.to == task.uid)
        {
            if let Some(entry) = self.entry_by_uid(&edge.from).await? {
                found.insert(edge.from, entry);
            }
        }
        for entry in self.tasks_with(PRECEDES, &task.uid).await? {
            found.insert(entry.task.uid.clone(), entry);
        }
        Ok(found.into_values().collect())
    }

    /// `start` and everything that waits for it, directly or not.
    async fn reachable(&self, start: &TaskEntry) -> Result<BTreeMap<String, TaskEntry>> {
        let mut seen: BTreeMap<String, TaskEntry> = BTreeMap::new();
        seen.insert(start.task.uid.clone(), start.clone());
        let mut work = VecDeque::from([start.clone()]);
        while let Some(entry) = work.pop_front() {
            for next in self.successors(&entry.task).await? {
                if !seen.contains_key(&next.task.uid) {
                    if seen.len() >= MAX_GRAPH {
                        return Err(Error::Conflict(format!(
                            "the dependency graph behind this task has more than {MAX_GRAPH} tasks"
                        )));
                    }
                    seen.insert(next.task.uid.clone(), next.clone());
                    work.push_back(next);
                }
            }
        }
        Ok(seen)
    }

    pub(super) async fn readiness_inner(&self, id: ObjectId) -> Result<Readiness> {
        let entry = self.entry(id).await?.ok_or(Error::NotFound(id))?;
        self.readiness_of_entry(&entry).await
    }

    pub(super) async fn readiness_of_entry(&self, entry: &TaskEntry) -> Result<Readiness> {
        let mut tasks: BTreeMap<String, Task> = self
            .reachable(entry)
            .await?
            .into_iter()
            .map(|(uid, e)| (uid, e.task))
            .collect();
        for p in self.predecessors(&entry.task).await? {
            tasks.entry(p.task.uid.clone()).or_insert(p.task);
        }
        let all: Vec<Task> = tasks.into_values().collect();
        Ok(readiness_of(&all)
            .remove(&entry.task.uid)
            .unwrap_or(Readiness::Ready))
    }

    pub(super) async fn blocked_by_inner(&self, id: ObjectId) -> Result<Vec<TaskEntry>> {
        let (Readiness::Blocked(by) | Readiness::DependencyCancelled(by)) =
            self.readiness_inner(id).await?
        else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for uid in by {
            if let Some(entry) = self.entry_by_uid(&uid).await? {
                out.push(entry);
            }
        }
        Ok(out)
    }

    pub(super) async fn dependents_inner(&self, id: ObjectId) -> Result<Vec<TaskEntry>> {
        let entry = self.entry(id).await?.ok_or(Error::NotFound(id))?;
        self.successors(&entry.task).await
    }

    /// Refuse the relation if it would close a dependency cycle. The edge `holder` -> `other_uid` is looked up through
    /// the tasks behind its successor: if its predecessor is among them, the way round is the error.
    pub(super) async fn check_no_cycle(
        &self,
        holder: &Task,
        other_uid: &str,
        relation: Relation,
    ) -> Result<()> {
        let Some(new) =
            Edge::of_relation(&holder.uid, other_uid, relation, chrono::Duration::zero())
        else {
            return Ok(());
        };
        let mut edges: Vec<Edge> = Vec::new();
        if let Some(start) = self.entry_by_uid(&new.to).await? {
            let behind = self.reachable(&start).await?;
            let mut seen = BTreeSet::new();
            for entry in behind.values() {
                for edge in edges_of(std::slice::from_ref(&entry.task)) {
                    if edge.from == entry.task.uid && seen.insert(edge.clone()) {
                        edges.push(edge);
                    }
                }
                for dependent in self.tasks_with(DEPENDS_ON, &entry.task.uid).await? {
                    for edge in edges_of(std::slice::from_ref(&dependent.task)) {
                        if edge.from == entry.task.uid && seen.insert(edge.clone()) {
                            edges.push(edge);
                        }
                    }
                }
            }
        }
        match path_closing_cycle(&edges, &new) {
            Some(path) => Err(Error::Cycle(path)),
            None => Ok(()),
        }
    }

    /// Is the task ready (`wanted = true`) or not (`false`)?
    async fn matches_ready(&self, entry: &TaskEntry, wanted: bool) -> Result<bool> {
        Ok((self.readiness_of_entry(entry).await? == Readiness::Ready) == wanted)
    }

    /// The tasks of a query with `ready` set: the store query without it, then each task asked.
    pub(super) async fn query_ready(
        &self,
        query: &TaskQuery,
        wanted: bool,
    ) -> Result<Vec<TaskEntry>> {
        let inner = inner_query(query, wanted);
        let mut out = Vec::new();
        for entry in self.query_inner(&inner).await? {
            if self.matches_ready(&entry, wanted).await? {
                out.push(entry);
            }
        }
        Ok(out)
    }

    /// A page of a query with `ready` set: reads the store's pages in chunks and keeps the matching tasks until the
    /// page is full, so the place a page ends at is still a place in the store's order.
    pub(super) async fn page_ready(
        &self,
        query: &TaskQuery,
        wanted: bool,
        place: Option<&Place>,
        size: usize,
    ) -> Result<TaskPage> {
        let inner = inner_query(query, wanted);
        let mut cursor: Option<Place> = place.cloned();
        let mut items: Vec<Placed> = Vec::new();
        loop {
            let page = self.page_inner(&inner, cursor.as_ref(), CHUNK).await?;
            for placed in page.items {
                cursor = Some(placed.place.clone());
                if self.matches_ready(&placed.entry, wanted).await? {
                    if items.len() == size {
                        return Ok(TaskPage { items, more: true });
                    }
                    items.push(placed);
                }
            }
            if !page.more {
                return Ok(TaskPage { items, more: false });
            }
        }
    }
}

/// The query as the store sees it: no `ready`, and only open tasks when ready ones are wanted (a finished task is not).
fn inner_query(query: &TaskQuery, wanted: bool) -> TaskQuery {
    let mut inner = query.clone();
    inner.ready = None;
    if wanted {
        inner.open = true;
    }
    inner
}
