//! The two views as one source: a `Layout` decides what rows the picture becomes, the rest is shared.
//!
//! Pattern: Strategy (`Layout`) inside an Adapter (`PlanSource`) over the existing `TaskAdapter` — reading comes
//! from the picture (`Cache`), so the window never waits for the core; a change goes to the adapter and then
//! renews the picture. Details, forms, saving and deleting are the adapter's, unchanged.

use std::sync::Arc;

use fs_components_workbenches::SourceError;
use fs_components_workbenches::tasks::{
    RowQuery, TaskCapabilities, TaskColumn, TaskDraft, TaskField, TaskRow, TaskSection, TaskSheet,
    TaskSource,
};

use super::cache::Cache;
use super::look::{Standing, standing, why};
use super::snapshot::Snapshot;
use crate::source::TaskAdapter;

/// What a view decides: its columns, its rows, what it can do, and what a row of its own opens.
pub trait Layout: Send + Sync {
    /// The columns of the status view; the list view ignores them.
    fn columns(&self) -> Vec<TaskColumn>;

    /// At most `limit` rows after `offset` of the whole answer to `query`, built from `snapshot`.
    fn rows(
        &self,
        snapshot: &Snapshot,
        query: &RowQuery,
        offset: usize,
        limit: usize,
    ) -> Vec<TaskRow>;

    /// The sheet of a row that is no task (a group), if `key` names one.
    fn own_sheet(&self, _key: &str, _snapshot: &Snapshot) -> Option<TaskSheet> {
        None
    }

    /// The form for a new task, if this view has a shorter one than the full form.
    fn blank(&self) -> Option<TaskDraft> {
        None
    }

    /// What this view lets the person do, from what the source can.
    fn capabilities(&self, can: TaskCapabilities) -> TaskCapabilities {
        can
    }
}

/// The adapter seen through a layout.
pub struct PlanSource {
    inner: Arc<TaskAdapter>,
    cache: Arc<Cache>,
    layout: Box<dyn Layout>,
}

impl PlanSource {
    /// `inner` shown by `layout`, reading from `cache`.
    pub fn new(inner: Arc<TaskAdapter>, cache: Arc<Cache>, layout: Box<dyn Layout>) -> Self {
        Self {
            inner,
            cache,
            layout,
        }
    }

    fn picture(&self) -> Result<Arc<Snapshot>, SourceError> {
        self.cache.current().ok_or_else(|| {
            self.cache.problem().map_or_else(
                || SourceError::failed("Reading the tasks and what blocks them ... one moment."),
                SourceError::failed,
            )
        })
    }

    fn after<T>(&self, result: Result<T, SourceError>) -> Result<T, SourceError> {
        if result.is_ok() {
            self.cache.changed();
        }
        result
    }
}

/// The readiness of a task as a section of its sheet.
fn readiness_section(snapshot: &Snapshot, key: &str) -> Option<TaskSection> {
    let entry = snapshot.entry(key)?;
    let word = match standing(snapshot, entry) {
        Standing::Today => "Ready, due today or earlier",
        Standing::Ready => "Ready",
        Standing::Blocked => "Blocked",
        Standing::Finished => "Finished",
    };
    let mut fields = vec![TaskField::new("Standing", word)];
    if let Some((_, reason)) = snapshot.readiness(key).and_then(|r| why(snapshot, r)) {
        fields.push(TaskField::new("Why", reason));
    }
    Some(TaskSection::new("Readiness", fields))
}

impl TaskSource for PlanSource {
    fn capabilities(&self) -> TaskCapabilities {
        self.layout.capabilities(self.inner.capabilities())
    }

    fn columns(&self) -> Vec<TaskColumn> {
        self.layout.columns()
    }

    fn rows(
        &self,
        query: &RowQuery,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<TaskRow>, SourceError> {
        let snapshot = self.picture()?;
        Ok(self.layout.rows(&snapshot, query, offset, limit))
    }

    fn read(&self, key: &str) -> Result<TaskSheet, SourceError> {
        let snapshot = self.picture()?;
        if let Some(sheet) = self.layout.own_sheet(key, &snapshot) {
            return Ok(sheet);
        }
        let mut sheet = self.inner.read(key)?;
        sheet.sections.extend(readiness_section(&snapshot, key));
        Ok(sheet)
    }

    fn check(&self, key: &str, text: &str) -> Option<String> {
        self.inner.check(key, text)
    }

    fn draft(&self, key: &str) -> Result<TaskDraft, SourceError> {
        self.inner.draft(key)
    }

    fn blank(&self) -> Result<TaskDraft, SourceError> {
        self.layout.blank().map_or_else(|| self.inner.blank(), Ok)
    }

    fn save(&self, key: &str, draft: &TaskDraft) -> Result<(), SourceError> {
        self.after(self.inner.save(key, draft))
    }

    fn create(&self, draft: &TaskDraft) -> Result<String, SourceError> {
        self.after(self.inner.create(draft))
    }

    fn delete(&self, key: &str) -> Result<(), SourceError> {
        self.after(self.inner.delete(key))
    }

    fn move_to(&self, key: &str, column: &str, revision: Option<&str>) -> Result<(), SourceError> {
        self.after(self.inner.move_to(key, column, revision))
    }
}
