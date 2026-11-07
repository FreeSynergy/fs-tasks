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
use fs_tasks_core::Task;
use fs_tasks_core::validate::PROGRESS;

use super::adapter::{TaskAdapter, reason, revision_in};
use super::apply::{apply_draft, set_progress};
use super::draft::{blank_draft, draft_of, is_series};
use super::rows::row_of;
use super::sheet::sheet_of;
use crate::words::{progress_title, say};

impl TaskSource for TaskAdapter {
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

    /// Read on from the cursor of the last call (`reading.rs`): the contract pages the list, the app
    /// filters it by text and by "open".
    fn rows(
        &self,
        query: &RowQuery,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<TaskRow>, SourceError> {
        let found = self.read_rows(query, offset, limit);
        Ok(found.map_err(|e| reason(&e))?.iter().map(row_of).collect())
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
        let expected = revision_in(draft.revision.as_deref())?;
        let read = self.entry(key)?;
        if read.revision != expected {
            return Err(SourceError::conflict(say::CHANGED_SINCE));
        }
        let entry = read.entry;
        let mut task = entry.task;
        apply_draft(&mut task, draft)?;
        self.runtime
            .block_on(self.items.edit_checked(entry.id, expected, task))
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
        let entry = self.entry(key)?.entry;
        self.runtime
            .block_on(self.items.delete(entry.id))
            .map_err(|e| reason(&e))
    }

    fn move_to(&self, key: &str, column: &str, revision: Option<&str>) -> Result<(), SourceError> {
        let expected = revision_in(revision)?;
        let read = self.entry(key)?;
        if read.revision != expected {
            return Err(SourceError::conflict(say::CHANGED_SINCE));
        }
        let entry = read.entry;
        if is_series(&entry.task) {
            return Err(SourceError::failed(say::SERIES_MOVE));
        }
        if entry.task.effective_progress() == column {
            return Ok(());
        }
        let mut task = entry.task;
        set_progress(&mut task, column)?;
        self.runtime
            .block_on(self.items.edit_checked(entry.id, expected, task))
            .map(|_| ())
            .map_err(|e| reason(&e))
    }
}
