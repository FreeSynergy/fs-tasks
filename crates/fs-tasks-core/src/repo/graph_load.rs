//! Loading the part of the dependency graph a question needs.
//!
//! Pattern: Repository (read side) with two strategies — a few tasks are followed through the index one step at a
//! time (an exact index lookup is one cheap store query); many tasks, or a graph that is deep, are read by one scan of
//! the store, which costs the same however deep the graph is. The pure functions of `plan` then work on what was
//! loaded. (Measured: an exact lookup 14 ms, a scan 0.3 s per 1000 tasks, an index condition over many rows or an OR
//! group of 50 values 0.7 to 1.2 s, all in a debug build.)

use std::collections::{BTreeMap, BTreeSet};

use fs_objects::Query;

use super::consts::{CONTEXT, TASK_TYPE};
use super::crud::entry_of;
use super::deps::{DEPENDS_ON, PRECEDES};
use super::handle::TaskRepo;
use crate::error::{Error, Result};
use crate::interface::TaskEntry;
use crate::model::Task;
use crate::plan::edges_of;

/// From this many start tasks on, the graph is read by a scan instead of being followed.
const SMALL: usize = 20;
/// Levels followed through the index; a deeper graph (a long chain) is read by a scan.
const MAX_LEVELS: usize = 3;
/// Tasks read per scan query.
const CHUNK: usize = 1000;
/// Most tasks one question loads; more is refused, so a huge or hostile graph cannot hold the store.
pub(super) const MAX_LOADED: usize = 20_000;
/// Most tasks of the store a scan reads (a store with more is refused for this question).
const MAX_SCANNED: usize = 100_000;

fn too_big() -> Error {
    Error::Conflict(format!(
        "the dependency graph behind these tasks has more than {MAX_LOADED} tasks"
    ))
}

impl TaskRepo {
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

    /// Every task of the store as the stored object (index included), read by one scan. A store with more than
    /// `MAX_SCANNED` tasks is refused.
    pub(super) async fn scan_objects(&self) -> Result<Vec<fs_objects::StoredObject>> {
        let mut all = Vec::new();
        loop {
            let q = Query::new()
                .of_type(TASK_TYPE)
                .in_context(CONTEXT)
                .limit(CHUNK)
                .offset(all.len());
            let chunk = self.queries.query(&q).await?;
            if chunk.is_empty() {
                return Ok(all);
            }
            all.extend(chunk);
            if all.len() > MAX_SCANNED {
                return Err(Error::Conflict(format!(
                    "the store has more than {MAX_SCANNED} tasks; the dependency graph cannot be read as a whole"
                )));
            }
        }
    }

    /// Every task that holds a dependency and every task one of them names: the whole graph of the store, read by one scan.
    pub(super) async fn scan_graph(&self) -> Result<BTreeMap<String, Task>> {
        let mut holders: BTreeMap<String, Task> = BTreeMap::new();
        let mut others: BTreeMap<String, Task> = BTreeMap::new();
        let mut offset = 0;
        loop {
            let q = Query::new()
                .of_type(TASK_TYPE)
                .in_context(CONTEXT)
                .limit(CHUNK)
                .offset(offset);
            let chunk = self.queries.query(&q).await?;
            if chunk.is_empty() {
                break;
            }
            offset += chunk.len();
            if offset > MAX_SCANNED {
                return Err(Error::Conflict(format!(
                    "the store has more than {MAX_SCANNED} tasks; the dependency graph cannot be read as a whole"
                )));
            }
            for object in chunk {
                let holds =
                    object.index.contains_key(PRECEDES) || object.index.contains_key(DEPENDS_ON);
                let task = entry_of(object)?.task;
                if holds {
                    holders.insert(task.uid.clone(), task);
                } else {
                    others.insert(task.uid.clone(), task);
                }
            }
        }
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
        if holders.len() > MAX_LOADED {
            return Err(too_big());
        }
        Ok(holders)
    }

    /// `start` and everything that waits for them, directly or not, as tasks by `uid`; with `predecessors`, also what the
    /// `start` tasks wait for (one step back: that is all a readiness needs). Edges are held by one of their ends, so
    /// both ends are asked for: the targets a loaded task names, and the tasks that name a loaded one.
    pub(super) async fn load_graph(
        &self,
        start: &[TaskEntry],
        predecessors: bool,
    ) -> Result<BTreeMap<String, Task>> {
        let mut seen: BTreeMap<String, Task> = start
            .iter()
            .map(|e| (e.task.uid.clone(), e.task.clone()))
            .collect();
        if start.len() > SMALL {
            return self.merged_with_scan(seen).await;
        }
        let mut frontier: Vec<Task> = seen.values().cloned().collect();
        let mut level = 0;
        while !frontier.is_empty() {
            level += 1;
            if level > MAX_LEVELS {
                return self.merged_with_scan(seen).await;
            }
            let uids: Vec<String> = frontier.iter().map(|t| t.uid.clone()).collect();
            let named = named_by(&frontier, &seen, |e, t| e.from == t.uid, |e| e.to.clone());
            let mut found = self.entries_by_uids(&named).await?;
            found.extend(self.with_any(DEPENDS_ON, &uids).await?);
            frontier = Vec::new();
            for entry in found {
                if !seen.contains_key(&entry.task.uid) {
                    if seen.len() >= MAX_LOADED {
                        return Err(too_big());
                    }
                    seen.insert(entry.task.uid.clone(), entry.task.clone());
                    frontier.push(entry.task);
                }
            }
        }
        if predecessors {
            let tasks: Vec<Task> = start.iter().map(|e| e.task.clone()).collect();
            let uids: Vec<String> = tasks.iter().map(|t| t.uid.clone()).collect();
            let named = named_by(&tasks, &seen, |e, t| e.to == t.uid, |e| e.from.clone());
            let mut found = self.entries_by_uids(&named).await?;
            found.extend(self.with_any(PRECEDES, &uids).await?);
            for entry in found {
                seen.entry(entry.task.uid.clone()).or_insert(entry.task);
            }
        }
        Ok(seen)
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
    keep: impl Fn(&crate::plan::Edge, &Task) -> bool,
    end: impl Fn(&crate::plan::Edge) -> String,
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
