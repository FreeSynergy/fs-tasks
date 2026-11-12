//! Replacing a task, with `sequence` handling, and the small edits built on it.
//!
//! Pattern: Template method — read the old task, check identity, bump `sequence` when a
//! scheduling-relevant field changed, rebuild, write, announce. `assign`, `relate` and `label`
//! are Commands (`Task::assign` ...) applied through the same flow.

use fs_objects::ObjectId;

use super::crud::entry_of;
use super::handle::TaskRepo;
use crate::error::{Error, Result};
use crate::interface::{TaskChange, TaskEntry};
use crate::model::Task;
use crate::now_utc;

impl TaskRepo {
    pub(super) async fn edit_inner(&self, id: ObjectId, mut new: Task) -> Result<TaskEntry> {
        new.normalise();
        let old_stored = self.stored(id).await?.ok_or(Error::NotFound(id))?;
        let old = Task::from_value(old_stored.content.clone())?;
        if old.uid != new.uid {
            return Err(Error::Conflict("the uid of a task cannot change".into()));
        }
        bump_sequence(&old, &mut new);
        let mut o = self.build(&new, Some(&old_stored.envelope), None)?;
        // keep non-derived index fields of the original import
        if let Some(v) = old_stored.index.get("tzdb_version") {
            o.index.insert("tzdb_version".into(), v.clone());
        }
        self.objects.put(o, &self.actor).await?;
        self.feed.notify(&TaskChange::Updated(id));
        let entry = self.entry(id).await?.ok_or(Error::NotFound(id))?;
        if old.effective_progress() != "completed" && entry.task.effective_progress() == "completed"
        {
            self.announce_unblocked(&entry).await;
        }
        Ok(entry)
    }

    /// Tell the listeners which tasks became ready because `done` was completed. A failed lookup tells nothing
    /// (the edit is written; the readiness of each task can still be asked).
    async fn announce_unblocked(&self, done: &crate::interface::TaskEntry) {
        let Ok(successors) = self.successors(&done.task).await else {
            return;
        };
        for next in successors {
            if matches!(
                self.readiness_of_entry(&next).await,
                Ok(crate::plan::Readiness::Ready)
            ) {
                self.feed.notify(&TaskChange::Unblocked(next.id));
            }
        }
    }

    /// Read the task, apply one command to it and store the result.
    pub(super) async fn modify(
        &self,
        id: ObjectId,
        command: impl FnOnce(&mut Task) + Send,
    ) -> Result<TaskEntry> {
        let stored = self.stored(id).await?.ok_or(Error::NotFound(id))?;
        let mut task = entry_of(stored)?.task;
        command(&mut task);
        self.edit_inner(id, task).await
    }
}

/// Set `updated`, keep `created`, and increment `sequence` on scheduling-relevant changes.
fn bump_sequence(old: &Task, new: &mut Task) {
    new.updated = Some(now_utc());
    new.created = new.created.take().or_else(|| old.created.clone());
    let changed = (&old.start, &old.due, &old.time_zone) != (&new.start, &new.due, &new.time_zone);
    if changed && new.sequence.unwrap_or(0) <= old.sequence.unwrap_or(0) {
        new.sequence = Some(old.sequence.unwrap_or(0) + 1);
    }
}
