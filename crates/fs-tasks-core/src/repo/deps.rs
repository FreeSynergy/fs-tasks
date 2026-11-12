//! Dependencies in the store: readiness, the neighbours of a task, the cycle check of `relate`, the `ready` filter.
//!
//! Pattern: Facade over the pure functions of `plan` — this file only reads the tasks the question needs (the task, its
//! predecessors, what is reachable behind it) through the index fields `precedes` and `depends_on`, and hands them to
//! `plan`. The index is what makes a reverse lookup ("who waits for this task?") a query and not a scan.

use std::collections::{BTreeMap, BTreeSet};

use fs_objects::{ObjectId, Query};

use super::consts::{CONTEXT, TASK_TYPE};
use super::crud::entry_of;
use super::graph_load::Preds;
use super::handle::TaskRepo;
use crate::error::{Error, Result};
use crate::interface::TaskEntry;
use crate::model::{Relation, Task};
use crate::plan::{Edge, Readiness, edges_of, path_closing_cycle, readiness_of};

/// Index lookups the quick search for a cycle may use before the graph is read as a whole.
const SEARCH_BUDGET: usize = 60;

/// Index field: the `uid`s of the successors named by the temporal relations a task holds.
pub(super) const PRECEDES: &str = "precedes";
/// Index field: the `uid`s a task depends on (`depends-on`).
pub(super) const DEPENDS_ON: &str = "depends_on";
/// Index field present on every task whose dependency fields are written (older tasks lack it; `upgrade_index`).
pub(super) const REL_INDEX: &str = "rel_index";

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

    pub(super) async fn readiness_inner(&self, id: ObjectId) -> Result<Readiness> {
        let entry = self.entry(id).await?.ok_or(Error::NotFound(id))?;
        self.readiness_of_entry(&entry).await
    }

    /// The tasks `task` waits for (its predecessors): what it depends on, and the tasks that hold a temporal relation to
    /// it. Predecessors that are not in this store are not here.
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

    /// The readiness of every task in `entries` at once: the graph around them is loaded in batches, the cycles are found
    /// once, and each task is judged against what was loaded. The answer has one value for each `uid`.
    pub(super) async fn readiness_many_entries(
        &self,
        entries: &[TaskEntry],
    ) -> Result<BTreeMap<String, Readiness>> {
        if entries.is_empty() {
            return Ok(BTreeMap::new());
        }
        let loaded = self.load_graph(entries, Preds::Start).await?;
        let all: Vec<Task> = loaded.into_values().collect();
        let mut judged = readiness_of(&all);
        let wanted: BTreeSet<&str> = entries.iter().map(|e| e.task.uid.as_str()).collect();
        judged.retain(|uid, _| wanted.contains(uid.as_str()));
        Ok(judged)
    }

    pub(super) async fn readiness_of_entry(&self, entry: &TaskEntry) -> Result<Readiness> {
        Ok(self
            .readiness_many_entries(std::slice::from_ref(entry))
            .await?
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
        if new.from != new.to && !self.may_close_cycle(&new).await? {
            return Ok(());
        }
        // A cycle is possible (or the search was too long to be sure): the graph is read as a whole, which also gives the
        // way round for the error.
        let graph: Vec<Task> = self.scan_graph().await?.into_values().collect();
        let edges = edges_of(&graph);
        match path_closing_cycle(&edges, &new) {
            Some(path) => Err(Error::Cycle(path)),
            None => Ok(()),
        }
    }

    /// A search from both ends: forward from the successor of `new` and backward from its predecessor, one step at a time
    /// on the side with fewer open ends. If one side runs out of tasks, no cycle can be closed (a new task in front of a
    /// long chain has nothing behind it and is answered at once); if the two sides meet, or the budget of index lookups
    /// is spent, a cycle is possible and the caller reads the whole graph.
    async fn may_close_cycle(&self, new: &Edge) -> Result<bool> {
        let (Some(from), Some(to)) = (
            self.entry_by_uid(&new.from).await?,
            self.entry_by_uid(&new.to).await?,
        ) else {
            return Ok(false); // an end the store does not hold has no way on from it
        };
        let mut forward: (BTreeSet<String>, Vec<TaskEntry>) =
            (BTreeSet::from([to.task.uid.clone()]), vec![to]);
        let mut backward: (BTreeSet<String>, Vec<TaskEntry>) =
            (BTreeSet::from([from.task.uid.clone()]), vec![from]);
        let mut lookups = 0;
        let mut forward_turn = true;
        while !forward.1.is_empty() && !backward.1.is_empty() {
            if forward.0.intersection(&backward.0).next().is_some() {
                return Ok(true);
            }
            if lookups >= SEARCH_BUDGET {
                return Ok(true);
            }
            // the side with fewer open ends goes first; when they are equal the sides take turns
            let step_forward = match forward.1.len().cmp(&backward.1.len()) {
                std::cmp::Ordering::Less => true,
                std::cmp::Ordering::Greater => false,
                std::cmp::Ordering::Equal => {
                    forward_turn = !forward_turn;
                    forward_turn
                }
            };
            let side = if step_forward {
                &mut forward
            } else {
                &mut backward
            };
            let open = std::mem::take(&mut side.1);
            for entry in open {
                let next = if step_forward {
                    self.successors(&entry.task).await?
                } else {
                    self.predecessors(&entry.task).await?
                };
                lookups += 1 + entry.task.related_to.len();
                for n in next {
                    if side.0.insert(n.task.uid.clone()) {
                        side.1.push(n);
                    }
                }
            }
        }
        // one side has run out: it was explored completely, so the two sides met or no cycle is possible
        Ok(!forward.0.is_disjoint(&backward.0))
    }
}
