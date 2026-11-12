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

use super::deps::DEPENDS_ON;
use super::graph_load::Preds;
use super::handle::TaskRepo;
use crate::interface::{TaskChange, TaskEntry};
use crate::model::Task;
use crate::plan::{Readiness, edges_of, readiness_of};

/// Created tasks asked one by one through the index up to this many, more by one scan of the graph.
const SMALL_CREATED: usize = 20;

/// Can `old` becoming `new` (an edit) or `old` going (`new` is `None`) free anybody? A change of progress can, a deletion
/// can, and a relation that is taken away can. A title or a label cannot, and neither can a relation that is only added
/// (it can only make a task wait) or whose gap changes (the gap does not decide readiness): those edits cost nothing here.
fn may_change_readiness(old: &Task, new: Option<&Task>) -> bool {
    let Some(new) = new else { return true };
    if old.effective_progress() != new.effective_progress() {
        return true;
    }
    let ties = |t: &Task| -> std::collections::BTreeSet<(String, String, crate::plan::EdgeKind)> {
        edges_of(std::slice::from_ref(t))
            .into_iter()
            .map(|e| (e.from, e.to, e.kind))
            .collect()
    };
    !ties(old).is_subset(&ties(new))
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

/// A change waiting to be judged: the task as it was before the change (`None`: it was just created), and its `uid`. What it
/// is now is read from the store when the change is judged (a later write may have changed it again).
pub(crate) struct Pending {
    old: Option<Task>,
    uid: String,
}

/// Announced ready and not seen waiting since: a second judgement of the same freeing (two writers freeing one task at
/// once, each judged against the store as it is) does not announce it again. Cleared when it grows past this many.
const MAX_ANNOUNCED: usize = 10_000;

impl TaskRepo {
    /// Run a write under the write turn; afterwards, with the turn given back, judge and announce who became ready.
    pub(super) async fn writing<T>(
        &self,
        run: impl AsyncFnOnce() -> crate::error::Result<T>,
    ) -> crate::error::Result<T> {
        let (out, queued_any) = {
            let _turn = self.writes.lock().await;
            let before = self.queued.load(std::sync::atomic::Ordering::SeqCst);
            let out = run().await;
            // the turn is ours: every change queued meanwhile was queued by this call
            (
                out,
                self.queued.load(std::sync::atomic::Ordering::SeqCst) != before,
            )
        };
        // a write that queued nothing has nothing to wait for (and does not wait for a judgement of somebody else)
        if queued_any {
            self.flush_unblocked().await;
        }
        out
    }

    /// Queue a change for [`Self::flush_unblocked`] if it can change anybody's readiness. Called under the write turn.
    pub(super) fn queue_unblocked(&self, old: &Task, new: Option<&TaskEntry>) {
        if let Some(new) = new {
            self.forget_newly_waiting(Some(old), &new.task);
        }
        if may_change_readiness(old, new.map(|e| &e.task)) {
            self.push_pending(Pending {
                old: Some(old.clone()),
                uid: old.uid.clone(),
            });
        }
    }

    /// Queue a task that was just created: a task that named it as a dependency may be free now. Whether any does is
    /// asked when the changes are judged (in one scan for an import), not for each created task.
    pub(super) fn queue_created(&self, task: &Task) {
        self.forget_newly_waiting(None, task);
        self.push_pending(Pending {
            old: None,
            uid: task.uid.clone(),
        });
    }

    /// A relation that was added can make its successor wait again: that task is no longer "announced ready", so freeing
    /// it later is announced again (the judgement only sees changes that can free somebody, not these).
    fn forget_newly_waiting(&self, old: Option<&Task>, new: &Task) {
        let before = old
            .map(|o| edges_of(std::slice::from_ref(o)))
            .unwrap_or_default();
        let added: Vec<String> = edges_of(std::slice::from_ref(new))
            .into_iter()
            .filter(|e| {
                !before
                    .iter()
                    .any(|b| b.from == e.from && b.to == e.to && b.kind == e.kind)
            })
            .map(|e| e.to)
            .collect();
        if added.is_empty() {
            return;
        }
        let mut announced = self
            .announced_ready
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for uid in added {
            announced.remove(&uid);
        }
    }

    fn push_pending(&self, change: Pending) {
        self.queued
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(change);
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
        let (created, changed): (Vec<Pending>, Vec<Pending>) =
            queued.into_iter().partition(|c| c.old.is_none());
        for change in changed {
            self.announce_unblocked(change.old.as_ref(), &change.uid)
                .await;
        }
        if created.is_empty() {
            return;
        }
        let uids: Vec<String> = created.into_iter().map(|c| c.uid).collect();
        let Ok(named) = self.named_as_dependency(&uids).await else {
            return;
        };
        for uid in named {
            self.announce_unblocked(None, &uid).await;
        }
    }

    /// Of these `uid`s, those that some stored task depends on (`depends-on`): creating such a task can free the one that
    /// waited. A few are asked one by one through the index, many by one scan of the graph.
    async fn named_as_dependency(&self, uids: &[String]) -> crate::error::Result<Vec<String>> {
        if uids.len() <= SMALL_CREATED {
            let mut out = Vec::new();
            for uid in uids {
                if !self.tasks_with(DEPENDS_ON, uid).await?.is_empty() {
                    out.push(uid.clone());
                }
            }
            return Ok(out);
        }
        let wanted: std::collections::BTreeSet<&str> = uids.iter().map(String::as_str).collect();
        let named: std::collections::BTreeSet<String> = self
            .scan_graph()
            .await?
            .values()
            .flat_map(Task::relations)
            .filter(|(_, r)| *r == crate::model::Relation::DependsOn)
            .map(|(other, _)| other)
            .collect();
        Ok(uids
            .iter()
            .filter(|u| wanted.contains(u.as_str()) && named.contains(*u))
            .cloned()
            .collect())
    }

    /// Announce the tasks that were waiting when `old` was the task (`None`: it did not exist) and are ready in the store
    /// as it is now (the task changed, was created or deleted). "Now" is read here, not taken from the writer: of two
    /// writers the later one decides. A failed lookup tells nothing: the write is done, and the readiness of each task can
    /// still be asked.
    pub(super) async fn announce_unblocked(&self, old: Option<&Task>, uid: &str) {
        let Ok(now) = self.entries_by_uids(&[uid.to_string()]).await else {
            return;
        };
        let current = now.into_iter().next();
        let Some(start) = old.or(current.as_ref().map(|c| &c.task)) else {
            return;
        };
        let Ok(mut graph) = self
            .load_graph_of(std::slice::from_ref(start), Preds::All)
            .await
        else {
            return;
        };
        // the closure of the new relations too, if the task has others than it had
        if let (Some(c), Some(old)) = (&current, old)
            && edges_of(std::slice::from_ref(&c.task)) != edges_of(std::slice::from_ref(old))
            && let Ok(more) = self
                .load_graph_of(std::slice::from_ref(&c.task), Preds::All)
                .await
        {
            for (u, task) in more {
                graph.entry(u).or_insert(task);
            }
        }
        // before: the task as it was (or not there); after: the store as it is
        match old {
            Some(o) => graph.insert(uid.to_string(), o.clone()),
            None => graph.remove(uid),
        };
        let before_tasks: Vec<Task> = graph.values().cloned().collect();
        match &current {
            Some(c) => graph.insert(uid.to_string(), c.task.clone()),
            None => graph.remove(uid),
        };
        let after_tasks: Vec<Task> = graph.values().cloned().collect();
        let (before, after) = (readiness_of(&before_tasks), readiness_of(&after_tasks));
        let names = self.not_announced(freed(&before, &after), &after, uid, current.is_none());
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

    /// Of the freed tasks, those not announced already; remembers them. A task seen waiting in `after` (or the task that
    /// is gone) is forgotten, so freeing it again later is announced again.
    fn not_announced(
        &self,
        freed: Vec<String>,
        after: &BTreeMap<String, Readiness>,
        changed: &str,
        gone: bool,
    ) -> Vec<String> {
        let mut announced = self
            .announced_ready
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if announced.len() > MAX_ANNOUNCED {
            announced.clear();
        }
        announced.retain(|u| after.get(u) == Some(&Readiness::Ready));
        if gone {
            announced.remove(changed);
        }
        freed
            .into_iter()
            .filter(|u| announced.insert(u.clone()))
            .collect()
    }
}
