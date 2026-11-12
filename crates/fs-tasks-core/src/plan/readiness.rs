//! Is a task ready to start, or what is it waiting for?
//!
//! Pattern: Specification — one small rule per edge kind, combined in a fixed order of precedence. Pure: it reads the
//! tasks and nothing else.

use std::collections::BTreeMap;

use super::graph::{Edge, EdgeKind, cycles, edges_of};
use crate::model::Task;

/// The state of a task as far as its dependencies say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readiness {
    /// Nothing keeps the task from starting.
    Ready,
    /// Waiting for these tasks (`uid`s); a person has nothing to decide, the order resolves it.
    Blocked(Vec<String>),
    /// A task it waits for is `cancelled` or `failed`: never freed automatically, a person decides (remove the
    /// dependency or plan the other task again).
    DependencyCancelled(Vec<String>),
    /// The task is finished (`completed`, `cancelled` or `failed`): nothing is left to wait for.
    Done,
    /// A dependency names a task this set does not hold (`uid`s): shown, never guessed.
    Unknown(Vec<String>),
    /// The task is part of a dependency cycle (the `uid`s of the group, sorted).
    Conflict(Vec<String>),
}

/// How far a task is, as the rules need it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    NotStarted,
    Started,
    Completed,
    Stopped,
}

fn stage(task: &Task) -> Stage {
    match Some(task.effective_progress()) {
        Some("completed") => Stage::Completed,
        Some("cancelled" | "failed") => Stage::Stopped,
        Some("in-process") => Stage::Started,
        _ => Stage::NotStarted,
    }
}

/// Readiness of every task of `tasks` (by `uid`), judged against the same set.
pub fn readiness_of(tasks: &[Task]) -> BTreeMap<String, Readiness> {
    let edges = edges_of(tasks);
    let by_uid: BTreeMap<&str, &Task> = tasks.iter().map(|t| (t.uid.as_str(), t)).collect();
    let in_cycle: BTreeMap<String, Vec<String>> = cycles(&edges)
        .into_iter()
        .flat_map(|group| {
            group
                .iter()
                .cloned()
                .map(|m| (m, group.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    tasks
        .iter()
        .map(|t| {
            let state = judge(t, &edges, &by_uid, &in_cycle);
            (t.uid.clone(), state)
        })
        .collect()
}

fn judge(
    task: &Task,
    edges: &[Edge],
    by_uid: &BTreeMap<&str, &Task>,
    in_cycle: &BTreeMap<String, Vec<String>>,
) -> Readiness {
    let own = stage(task);
    if matches!(own, Stage::Completed | Stage::Stopped) {
        return Readiness::Done;
    }
    if let Some(group) = in_cycle.get(&task.uid) {
        return Readiness::Conflict(group.clone());
    }
    let (mut waiting, mut stopped, mut missing) = (Vec::new(), Vec::new(), Vec::new());
    for edge in edges
        .iter()
        .filter(|e| e.to == task.uid && e.kind.blocks_start())
    {
        let Some(pred) = by_uid.get(edge.from.as_str()) else {
            missing.push(edge.from.clone());
            continue;
        };
        match (edge.kind, stage(pred)) {
            (_, Stage::Stopped) => stopped.push(edge.from.clone()),
            (EdgeKind::FinishToStart, Stage::Completed)
            | (EdgeKind::StartToStart, Stage::Started | Stage::Completed) => {}
            _ => waiting.push(edge.from.clone()),
        }
    }
    for list in [&mut waiting, &mut stopped, &mut missing] {
        list.sort();
        list.dedup();
    }
    if !stopped.is_empty() {
        Readiness::DependencyCancelled(stopped)
    } else if !waiting.is_empty() {
        Readiness::Blocked(waiting)
    } else if !missing.is_empty() {
        Readiness::Unknown(missing)
    } else {
        Readiness::Ready
    }
}
