//! What the plan views draw from: every task of the store with its readiness, judged once.
//!
//! Pattern: Memento — an immutable picture of the store at one moment. The views read only this, so drawing a
//! page never waits for the core; a newer picture is made in the background and replaces this one whole.

use std::collections::HashMap;
use std::sync::Arc;

use fs_tasks_core::plan::Readiness;
use fs_tasks_core::{Error, TaskEntry, TaskItems};

use crate::day::Day;

/// All tasks, their readiness (judged against the same set), and what a person needs to name them.
pub struct Snapshot {
    /// The tasks in the core's order: by due date, undated last.
    pub entries: Vec<TaskEntry>,
    /// The day the groups are cut at.
    pub today: Day,
    ready: HashMap<String, Readiness>,
    titles: HashMap<String, String>,
    index: HashMap<String, usize>,
}

impl Snapshot {
    /// Ask the core once for the tasks and once for the readiness of all of them (the set at once).
    ///
    /// # Errors
    /// The core's error, unchanged.
    pub async fn load(items: &Arc<dyn TaskItems>, today: Day) -> Result<Self, Error> {
        let entries = items.list().await?;
        let ids: Vec<_> = entries.iter().map(|e| e.id).collect();
        let judged = items.readiness_many(&ids).await?;
        let ready = judged
            .into_iter()
            .map(|(id, r)| (id.to_string(), r))
            .collect();
        Ok(Self::from_parts(entries, ready, today))
    }

    /// A picture from tasks and the readiness already worked out (tests and `load`).
    #[must_use]
    pub fn from_parts(
        entries: Vec<TaskEntry>,
        ready: HashMap<String, Readiness>,
        today: Day,
    ) -> Self {
        let titles = entries
            .iter()
            .map(|e| (e.task.uid.clone(), e.task.display_title()))
            .collect();
        let index = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.id.to_string(), i))
            .collect();
        Self {
            entries,
            today,
            ready,
            titles,
            index,
        }
    }

    /// The readiness of a task; a task the picture does not know yet is judged `Ready` only when it is new, so
    /// nothing is claimed about it: `None`.
    #[must_use]
    pub fn readiness(&self, key: &str) -> Option<&Readiness> {
        self.ready.get(key)
    }

    /// The task with this key.
    #[must_use]
    pub fn entry(&self, key: &str) -> Option<&TaskEntry> {
        self.index.get(key).map(|i| &self.entries[*i])
    }

    /// What to call the task with this `uid`: its title, or the `uid` itself for a task nobody holds.
    #[must_use]
    pub fn name_of<'a>(&'a self, uid: &'a str) -> &'a str {
        self.titles.get(uid).map_or(uid, String::as_str)
    }
}
