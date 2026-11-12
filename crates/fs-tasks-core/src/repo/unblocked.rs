//! Who became ready because of a change: the trigger of anyone who takes ready tasks.
//!
//! Pattern: Observer (the announcing side) — after a write that could change a readiness (a progress, a relation, a
//! deletion), the graph around the changed task is loaded once with the task as it was, readiness is judged before
//! (the old task) and after (the new task, or none), and every task that was not ready and is ready now is announced as
//! `Unblocked`. A task that was ready before, or finished, says nothing; a task freed because a cycle was resolved is
//! announced like any other. Judging both states from one load keeps the two answers about the same graph.

use std::collections::BTreeMap;

use super::graph_load::Preds;
use super::handle::TaskRepo;
use crate::interface::{TaskChange, TaskEntry};
use crate::model::Task;
use crate::plan::{Readiness, edges_of, readiness_of};

/// Can `old` becoming `new` (an edit) or `old` going (`new` is `None`) change anybody's readiness? A title or a label
/// cannot, so those edits cost nothing here.
fn may_change_readiness(old: &Task, new: Option<&Task>) -> bool {
    match new {
        None => true,
        Some(new) => {
            old.effective_progress() != new.effective_progress()
                || edges_of(std::slice::from_ref(old)) != edges_of(std::slice::from_ref(new))
        }
    }
}

/// The tasks that were waiting (blocked, unknown, in a cycle or behind a cancelled task) in `before` and are ready in
/// `after`.
fn freed(before: &BTreeMap<String, Readiness>, after: &BTreeMap<String, Readiness>) -> Vec<String> {
    after
        .iter()
        .filter(|(_, now)| **now == Readiness::Ready)
        .filter(|(uid, _)| {
            matches!(
                before.get(*uid),
                Some(
                    Readiness::Blocked(_)
                        | Readiness::Unknown(_)
                        | Readiness::Conflict(_)
                        | Readiness::DependencyCancelled(_)
                )
            )
        })
        .map(|(uid, _)| uid.clone())
        .collect()
}

impl TaskRepo {
    /// Announce the tasks freed by `old` becoming `new` (an edit) or by `old` being deleted (`new = None`). A failed
    /// lookup tells nothing: the write is done, and the readiness of each task can still be asked.
    pub(super) async fn announce_unblocked(&self, old: &Task, new: Option<&TaskEntry>) {
        let new_task = new.map(|e| &e.task);
        if !may_change_readiness(old, new_task) {
            return;
        }
        let Ok(mut graph) = self
            .load_graph_of(std::slice::from_ref(old), Preds::All)
            .await
        else {
            return;
        };
        // the store holds the new state; the graph is judged with the old task for "before"
        graph.insert(old.uid.clone(), old.clone());
        let before_tasks: Vec<Task> = graph.values().cloned().collect();
        match new_task {
            Some(n) => graph.insert(n.uid.clone(), n.clone()),
            None => graph.remove(&old.uid),
        };
        let after_tasks: Vec<Task> = graph.values().cloned().collect();
        let (before, after) = (readiness_of(&before_tasks), readiness_of(&after_tasks));
        let names = freed(&before, &after);
        if names.is_empty() {
            return;
        }
        let Ok(mut entries) = self.entries_by_uids(&names).await else {
            return;
        };
        if let Some(new) = new
            && names.contains(&new.task.uid)
            && !entries.iter().any(|e| e.id == new.id)
        {
            entries.push(new.clone());
        }
        for entry in entries {
            self.feed.notify(&TaskChange::Unblocked(entry.id));
        }
    }
}
