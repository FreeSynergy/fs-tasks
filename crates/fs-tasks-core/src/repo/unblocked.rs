//! Who became ready because of a change: the trigger of anyone who takes ready tasks.
//!
//! Pattern: Observer (the announcing side) — after a write that could change a readiness (a progress, a relation, a
//! deletion), the graph around the changed task is loaded once with the task as it was, readiness is judged before
//! (the old task) and after (the new task, or none), and every task that was not ready and is ready now is announced as
//! `Unblocked`. A write only queues the change (cheap, under the write turn); the judgement reads the graph and runs
//! after the turn is given back (`TaskRepo::writing`), so a deep graph does not make every other write wait. Under
//! concurrent writes the judgement may already include a later change of the same graph. A task that was ready before, or finished, says nothing; a task freed because a cycle was resolved is
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

/// A change waiting to be judged: the task as it was before the change. What it is now is read from the store when the
/// change is judged (a later write may have changed it again).
pub(crate) struct Pending {
    old: Task,
}

impl TaskRepo {
    /// Run a write under the write turn; afterwards, with the turn given back, judge and announce who became ready.
    pub(super) async fn writing<T>(
        &self,
        run: impl AsyncFnOnce() -> crate::error::Result<T>,
    ) -> crate::error::Result<T> {
        let out = {
            let _turn = self.writes.lock().await;
            run().await
        };
        self.flush_unblocked().await;
        out
    }

    /// Queue a change for [`Self::flush_unblocked`] if it can change anybody's readiness. Called under the write turn.
    pub(super) fn queue_unblocked(&self, old: &Task, new: Option<&TaskEntry>) {
        if may_change_readiness(old, new.map(|e| &e.task)) {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            pending.push(Pending { old: old.clone() });
        }
    }

    /// Judge and announce every queued change. One flush at a time: a call that queued a change returns only after the
    /// change was judged, by itself or by the flush that took it (that one finishes first, the next waits for it).
    pub(super) async fn flush_unblocked(&self) {
        let _one_flush = self.flush_turn.lock().await;
        let queued: Vec<Pending> = {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut *pending)
        };
        for change in queued {
            self.announce_unblocked(&change.old).await;
        }
    }

    /// Announce the tasks that were waiting when `old` was the task and are ready in the store as it is now (the task
    /// changed, or deleted). "Now" is read here, not taken from the writer: of two writers the later one decides. A
    /// failed lookup tells nothing: the write is done, and the readiness of each task can still be asked.
    pub(super) async fn announce_unblocked(&self, old: &Task) {
        let Ok(now) = self.entries_by_uids(std::slice::from_ref(&old.uid)).await else {
            return;
        };
        let current = now.into_iter().next();
        let Ok(mut graph) = self
            .load_graph_of(std::slice::from_ref(old), Preds::All)
            .await
        else {
            return;
        };
        // the closure of the new relations too, if the task has others than it had
        if let Some(c) = &current
            && edges_of(std::slice::from_ref(&c.task)) != edges_of(std::slice::from_ref(old))
            && let Ok(more) = self
                .load_graph_of(std::slice::from_ref(&c.task), Preds::All)
                .await
        {
            for (uid, task) in more {
                graph.entry(uid).or_insert(task);
            }
        }
        // before: the task as it was; after: the store as it is
        graph.insert(old.uid.clone(), old.clone());
        let before_tasks: Vec<Task> = graph.values().cloned().collect();
        match &current {
            Some(c) => graph.insert(c.task.uid.clone(), c.task.clone()),
            None => graph.remove(&old.uid),
        };
        let after_tasks: Vec<Task> = graph.values().cloned().collect();
        let (before, after) = (readiness_of(&before_tasks), readiness_of(&after_tasks));
        let names = freed(&before, &after);
        if names.is_empty() {
            return;
        }
        let Ok(entries) = self.entries_by_uids(&names).await else {
            return;
        };
        for entry in entries {
            self.feed.notify(&TaskChange::Unblocked(entry.id));
        }
    }
}
