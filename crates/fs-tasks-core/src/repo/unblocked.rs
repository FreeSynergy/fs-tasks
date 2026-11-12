//! Who became ready because of a change: the trigger of anyone who takes ready tasks.
//!
//! Pattern: Observer (the announcing side) — after a write, the tasks whose blocking edge was satisfied or removed are
//! asked once with `readiness_many`, and every one that is ready now is announced as `Unblocked`. A task is announced
//! only when this very change took away a blocker: a tie that was free before and after says nothing.

use std::collections::BTreeSet;

use super::handle::TaskRepo;
use crate::interface::{TaskChange, TaskEntry};
use crate::model::Task;
use crate::plan::{Readiness, edges_of};

/// The `uid`s that may have become ready because `task` went from `old` to `new` (the progress of `old` is that of the
/// task before the edit; `None` for a task that is gone).
fn freed_by(old: Option<&Task>, new: Option<&Task>, successors: &[Task]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let (old_progress, new_progress) = (
        old.map(Task::effective_progress),
        new.map(Task::effective_progress),
    );
    // 1. the tie to a successor is satisfied now and was not before (the task started or was completed), or the task
    //    is gone and with it the tie.
    let subject = new.or(old);
    if let Some(subject) = subject {
        for s in successors {
            for edge in edges_of(&[subject.clone(), s.clone()])
                .into_iter()
                .filter(|e| e.from == subject.uid && e.to == s.uid && e.kind.blocks_start())
            {
                let before = old_progress.is_some_and(|p| edge.kind.satisfied_by(p));
                let now = new_progress.is_none_or(|p| edge.kind.satisfied_by(p));
                if now && !before {
                    out.insert(s.uid.clone());
                }
            }
        }
    }
    // 2. an edge the task held is gone from the new version: the task it kept waiting is freed of it.
    if let (Some(old), Some(new)) = (old, new) {
        let kept = edges_of(std::slice::from_ref(new));
        for edge in edges_of(std::slice::from_ref(old)) {
            if edge.kind.blocks_start() && !kept.contains(&edge) {
                out.insert(edge.to);
            }
        }
    }
    out
}

impl TaskRepo {
    /// Announce the tasks freed by `old` becoming `new` (an edit) or by `old` being deleted (`new = None`).
    /// `successors` are the tasks that waited for it, read before a deletion. A failed lookup tells nothing: the write
    /// is done, and the readiness of each task can still be asked.
    pub(super) async fn announce_unblocked(
        &self,
        old: Option<&Task>,
        new: Option<&TaskEntry>,
        successors: Vec<Task>,
    ) {
        let candidates = freed_by(old, new.map(|e| &e.task), &successors);
        if candidates.is_empty() {
            return;
        }
        let names: Vec<String> = candidates.into_iter().collect();
        let Ok(mut entries) = self.entries_by_uids(&names).await else {
            return;
        };
        if let Some(new) = new {
            if names.contains(&new.task.uid) && !entries.iter().any(|e| e.id == new.id) {
                entries.push(new.clone());
            }
        }
        let Ok(judged) = self.readiness_many_entries(&entries).await else {
            return;
        };
        for entry in entries {
            if judged.get(&entry.task.uid) == Some(&Readiness::Ready) {
                self.feed.notify(&TaskChange::Unblocked(entry.id));
            }
        }
    }
}
