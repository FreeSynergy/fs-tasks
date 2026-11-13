//! `TaskAdapter` as the workbench sees it.
//!
//! Pattern: Adapter — each method of `TaskSource` in terms of the contract `TaskItems`. The form is
//! always of the stored task, and a save goes onto the task as it is stored now. A move changes nothing
//! but the status and the time it was chosen; a task that repeats is refused rather than damaged, and a
//! move to the status it effectively has writes nothing, so a computed status stays computed.

use fs_components_workbenches::SourceError;
use fs_components_workbenches::tasks::{
    RowQuery, TaskCapabilities, TaskColumn, TaskDraft, TaskRow, TaskSheet, TaskSource,
};
use fs_tasks_core::{Revision, Task};
use fs_tasks_core::validate::PROGRESS;

use super::adapter::{TaskAdapter, reason};
use super::apply::{apply_draft, due, set_progress};
use super::draft::{DUE, blank_draft, draft_of, is_series};
use super::rows::{is_open, matches, row_of};
use super::sheet::sheet_of;
use crate::words::{progress_title, say};

impl TaskSource for TaskAdapter {
    /// The due date is judged as it is typed, by the same rule as the save.
    fn check(&self, key: &str, text: &str) -> Option<String> {
        if key == DUE {
            return due(text.trim()).err().map(|e| e.message);
        }
        None
    }

    fn capabilities(&self) -> TaskCapabilities {
        let report = self.items.capabilities();
        let mut can = TaskCapabilities::read_only();
        can.edit = report.is_supported("task.edit");
        can.create = report.is_supported("task.create");
        can.delete = report.is_supported("task.delete");
        can.move_tasks = can.edit;
        can
    }

    fn columns(&self) -> Vec<TaskColumn> {
        PROGRESS
            .iter()
            .map(|p| TaskColumn::new(*p, progress_title(p)))
            .collect()
    }

    /// Cut from the whole list: `TaskItems` has no paging or text search yet.
    fn rows(
        &self,
        query: &RowQuery,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<TaskRow>, SourceError> {
        let all = self
            .runtime
            .block_on(self.items.list())
            .map_err(|e| reason(&e))?;
        let term = query.term.to_lowercase();
        Ok(all
            .iter()
            .filter(|e| matches(&e.task, &term))
            .filter(|e| !query.open_only || is_open(&e.task))
            .skip(offset)
            .take(limit)
            .map(row_of)
            .collect())
    }

    fn read(&self, key: &str) -> Result<TaskSheet, SourceError> {
        Ok(sheet_of(&self.entry(key)?))
    }

    fn draft(&self, key: &str) -> Result<TaskDraft, SourceError> {
        Ok(draft_of(&self.entry(key)?))
    }

    fn blank(&self) -> Result<TaskDraft, SourceError> {
        Ok(blank_draft())
    }

    fn save(&self, key: &str, draft: &TaskDraft) -> Result<(), SourceError> {
        let entry = self.entry_at(key, draft.revision.as_deref())?;
        let revision = Revision::of(&entry);
        let mut task = entry.task;
        apply_draft(&mut task, draft)?;
        // Compare and set: a change made between the reading and the writing (the command line) is a conflict.
        self.runtime
            .block_on(self.items.edit_checked(entry.id, revision, task))
            .map(|_| ())
            .map_err(|e| reason(&e))
    }

    fn create(&self, draft: &TaskDraft) -> Result<String, SourceError> {
        let mut task = Task::new("");
        task.title = None;
        apply_draft(&mut task, draft)?;
        if task.title.is_none() {
            return Err(SourceError::failed(say::NEEDS_TITLE));
        }
        self.runtime
            .block_on(self.items.create(task))
            .map(|entry| entry.id.to_string())
            .map_err(|e| reason(&e))
    }

    fn delete(&self, key: &str) -> Result<(), SourceError> {
        let entry = self.entry(key)?;
        self.runtime
            .block_on(self.items.delete(entry.id))
            .map_err(|e| reason(&e))
    }

    fn move_to(&self, key: &str, column: &str, revision: Option<&str>) -> Result<(), SourceError> {
        let entry = self.entry_at(key, revision)?;
        if is_series(&entry.task) {
            return Err(SourceError::failed(say::SERIES_MOVE));
        }
        if entry.task.effective_progress() == column {
            return Ok(());
        }
        let revision = Revision::of(&entry);
        let mut task = entry.task;
        set_progress(&mut task, column)?;
        self.runtime
            .block_on(self.items.edit_checked(entry.id, revision, task))
            .map(|_| ())
            .map_err(|e| reason(&e))
    }
}
