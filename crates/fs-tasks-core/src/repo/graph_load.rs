//! Loading the part of the dependency graph a question needs.
//!
//! Pattern: Repository (read side) with two strategies — a few tasks are followed through the index one step at a
//! time (an exact index lookup is one cheap store query); many tasks, or a graph that is deep, are read by one scan of
//! the store, which costs the same however deep the graph is. The pure functions of `plan` then work on what was
//! loaded. (Measured: an exact lookup 14 ms, a scan 0.3 s per 1000 tasks, an index condition over many rows or an OR
//! group of 50 values 0.7 to 1.2 s, all in a debug build.)
//!
//! **Rule for a task the store does not hold** (named by a relation, or gone): it is a node of the graph without
//! relations of its own, but the tasks that depend on it are followed, so a cycle through it is found by every way of
//! asking (one task, many tasks, the `ready` filter alike).
//!
//! **Limits** (`TaskRepo::with_graph_limits`): a question that follows the graph refuses to load more than `max_loaded`
//! tasks (default 20 000), and a scan refuses a store with more than `max_scanned` tasks (default 100 000). The scan
//! path of the `ready` filter checks the first limit on the tasks it has to parse (the tasks that hold a dependency and
//! those they name), not on the whole store.

use std::collections::{BTreeMap, BTreeSet};

use fs_objects::{Op, Query, StoredObject};

use super::consts::{CONTEXT, ID_KEY, TASK_TYPE};
use super::crud::entry_of;
use super::deps::{DEPENDS_ON, PRECEDES};
use super::handle::TaskRepo;
use crate::error::{Error, Result};
use crate::interface::TaskEntry;
use crate::model::Task;
use crate::plan::{Edge, edges_of};

/// From this many start tasks on, the graph is read by a scan instead of being followed.
const SMALL: usize = 20;
/// Levels followed through the index; a deeper graph (a long chain) is read by a scan.
const MAX_LEVELS: usize = 3;
/// Default of the tasks read per scan query.
pub(super) const DEFAULT_SCAN_CHUNK: usize = 1000;
/// Default of the most tasks one question loads.
pub(super) const DEFAULT_MAX_LOADED: usize = 20_000;
/// Default of the most tasks of the store a scan reads.
pub(super) const DEFAULT_MAX_SCANNED: usize = 100_000;

/// Which predecessors a load brings along.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Preds {
    /// Those of the start tasks (all a readiness needs).
    Start,
    /// Those of every task loaded (what a before-and-after comparison of a change needs).
    All,
}

impl TaskRepo {
    pub(super) fn too_big(&self) -> Error {
        Error::Conflict(format!(
            "the dependency graph behind these tasks has more than {} tasks",
            self.max_loaded
        ))
    }

    /// The tasks whose index `field` has any of `values`, one exact lookup each.
    async fn with_any(&self, field: &str, values: &[String]) -> Result<Vec<TaskEntry>> {
        let mut out = Vec::new();
        for value in values {
            let q = Query::new()
                .of_type(TASK_TYPE)
                .in_context(CONTEXT)
                .with_index(field, value);
            for object in self.queries.query(&q).await? {
                out.push(entry_of(object)?);
            }
        }
        Ok(out)
    }

    /// The tasks with these `uid`s (those the store does not hold are not in the answer).
    pub(super) async fn entries_by_uids(&self, uids: &[String]) -> Result<Vec<TaskEntry>> {
        self.with_any("uid", uids).await
    }

    /// Every task of the store, one stored object at a time, in the order of their ids. The scan goes on from the last
    /// id it saw (not by a count of rows), so an edit during the scan, which moves a task in the order of changes, neither
    /// hides a task nor shows it twice. A store with more than `max_scanned` tasks is refused, and so is one that has
    /// tasks without an id key (written before 0.7.0; `upgrade_index`).
    pub(super) async fn scan_each(
        &self,
        mut each: impl FnMut(StoredObject) -> Result<()>,
    ) -> Result<()> {
        let mut read = 0usize;
        let mut after: Option<String> = None;
        loop {
            let mut q = Query::new()
                .of_type(TASK_TYPE)
                .in_context(CONTEXT)
                .order_by_index(ID_KEY)
                .limit(self.scan_chunk);
            if let Some(id) = &after {
                q = q.index_op(ID_KEY, Op::Gt, id);
            }
            let chunk = self.queries.query(&q).await?;
            if chunk.is_empty() {
                break;
            }
            for object in chunk {
                read += 1;
                if read > self.max_scanned {
                    return Err(Error::Conflict(format!(
                        "the store has more than {} tasks; the dependency graph cannot be read as a whole",
                        self.max_scanned
                    )));
                }
                after = object.index.get(ID_KEY).and_then(|v| v.first()).cloned();
                each(object)?;
            }
            if after.is_none() {
                break;
            }
        }
        let all = self
            .queries
            .count(&Query::new().of_type(TASK_TYPE).in_context(CONTEXT))
            .await?;
        if read < all {
            return Err(Error::Stored(format!(
                "{} of {all} tasks have no id key (written before 0.7.0); run upgrade_index and read its report",
                all - read
            )));
        }
        Ok(())
    }

    /// Every task of the store as the stored object (index included).
    pub(super) async fn scan_objects(&self) -> Result<Vec<StoredObject>> {
        let mut all = Vec::new();
        self.scan_each(|o| {
            all.push(o);
            Ok(())
        })
        .await?;
        Ok(all)
    }

    /// Every task that holds a dependency and every task one of them names: the whole graph of the store, read by one
    /// scan; only these tasks are parsed and kept (at most `max_loaded`).
    pub(super) async fn scan_graph(&self) -> Result<BTreeMap<String, Task>> {
        let mut holders: BTreeMap<String, Task> = BTreeMap::new();
        let mut others: BTreeMap<String, Task> = BTreeMap::new();
        self.scan_each(|object| {
            let holds =
                object.index.contains_key(PRECEDES) || object.index.contains_key(DEPENDS_ON);
            let task = entry_of(object)?.task;
            if holds {
                holders.insert(task.uid.clone(), task);
            } else {
                others.insert(task.uid.clone(), task);
            }
            Ok(())
        })
        .await?;
        let named: BTreeSet<String> = holders
            .values()
            .flat_map(|t| {
                edges_of(std::slice::from_ref(t))
                    .into_iter()
                    .flat_map(|e| [e.from, e.to])
            })
            .collect();
        for (uid, task) in others {
            if named.contains(&uid) {
                holders.insert(uid, task);
            }
        }
        if holders.len() > self.max_loaded {
            return Err(self.too_big());
        }
        Ok(holders)
    }

    /// `start` and everything that waits for them, directly or not, as tasks by `uid`, with the predecessors `preds` asks
    /// for. Edges are held by one of their ends, so both ends are asked for: the targets a loaded task names, and the
    /// tasks that name a loaded one.
    pub(super) async fn load_graph(
        &self,
        start: &[TaskEntry],
        preds: Preds,
    ) -> Result<BTreeMap<String, Task>> {
        let tasks: Vec<Task> = start.iter().map(|e| e.task.clone()).collect();
        self.load_graph_of(&tasks, preds).await
    }

    /// [`Self::load_graph`] for tasks that need not be the stored ones (the task before an edit).
    pub(super) async fn load_graph_of(
        &self,
        start: &[Task],
        preds: Preds,
    ) -> Result<BTreeMap<String, Task>> {
        let mut seen: BTreeMap<String, Task> =
            start.iter().map(|t| (t.uid.clone(), t.clone())).collect();
        if start.len() > SMALL {
            return self.merged_with_scan(seen).await;
        }
        let mut frontier: Vec<Task> = seen.values().cloned().collect();
        let mut ghosts: Vec<String> = Vec::new();
        let mut known_ghosts: BTreeSet<String> = BTreeSet::new();
        let mut level = 0;
        while !frontier.is_empty() || !ghosts.is_empty() {
            level += 1;
            if level > MAX_LEVELS {
                return self.merged_with_scan(seen).await;
            }
            let mut uids: Vec<String> = frontier.iter().map(|t| t.uid.clone()).collect();
            uids.append(&mut ghosts);
            let named = named_by(&frontier, &seen, |e, t| e.from == t.uid, |e| e.to.clone());
            let mut found = self.entries_by_uids(&named).await?;
            // a name the store does not hold is a task without relations of its own, whose dependents still count
            let found_uids: BTreeSet<&str> = found.iter().map(|e| e.task.uid.as_str()).collect();
            for name in &named {
                if !found_uids.contains(name.as_str()) && known_ghosts.insert(name.clone()) {
                    ghosts.push(name.clone());
                }
            }
            found.extend(self.with_any(DEPENDS_ON, &uids).await?);
            frontier = Vec::new();
            for entry in found {
                if !seen.contains_key(&entry.task.uid) {
                    if seen.len() >= self.max_loaded {
                        return Err(self.too_big());
                    }
                    seen.insert(entry.task.uid.clone(), entry.task.clone());
                    frontier.push(entry.task);
                }
            }
        }
        match preds {
            Preds::Start => {
                self.add_predecessors(&mut seen, start).await?;
            }
            Preds::All => {
                if seen.len() > SMALL {
                    return self.merged_with_scan(seen).await;
                }
                let all: Vec<Task> = seen.values().cloned().collect();
                self.add_predecessors(&mut seen, &all).await?;
            }
        }
        Ok(seen)
    }

    /// The tasks `of` wait for: what they depend on, and the tasks that hold a temporal relation to them.
    async fn add_predecessors(&self, seen: &mut BTreeMap<String, Task>, of: &[Task]) -> Result<()> {
        let uids: Vec<String> = of.iter().map(|t| t.uid.clone()).collect();
        let named = named_by(of, seen, |e, t| e.to == t.uid, |e| e.from.clone());
        let mut found = self.entries_by_uids(&named).await?;
        found.extend(self.with_any(PRECEDES, &uids).await?);
        for entry in found {
            if !seen.contains_key(&entry.task.uid) && seen.len() >= self.max_loaded {
                return Err(self.too_big());
            }
            seen.entry(entry.task.uid.clone()).or_insert(entry.task);
        }
        Ok(())
    }

    /// `seen` plus the whole graph of the store (the start tasks stay as they were given).
    async fn merged_with_scan(
        &self,
        mut seen: BTreeMap<String, Task>,
    ) -> Result<BTreeMap<String, Task>> {
        for (uid, task) in self.scan_graph().await? {
            seen.entry(uid).or_insert(task);
        }
        Ok(seen)
    }
}

/// The `uid`s on the far end of the edges a task holds (picked by `keep`, read by `end`), that are not loaded yet.
fn named_by(
    tasks: &[Task],
    seen: &BTreeMap<String, Task>,
    keep: impl Fn(&Edge, &Task) -> bool,
    end: impl Fn(&Edge) -> String,
) -> Vec<String> {
    let mut named: Vec<String> = tasks
        .iter()
        .flat_map(|t| {
            edges_of(std::slice::from_ref(t))
                .into_iter()
                .filter(|e| keep(e, t))
                .map(|e| end(&e))
                .collect::<Vec<_>>()
        })
        .filter(|uid| !seen.contains_key(uid))
        .collect();
    named.sort();
    named.dedup();
    named
}
