// controller.rs — TaskController: domain logic via TaskStore strategy.
//
// Design Pattern: Strategy — TaskController holds a `dyn TaskStore`;
//                 swap the store to change persistence without touching
//                 any other layer (gRPC, REST, CLI, UI).

use std::sync::Arc;

use crate::model::TaskPipeline;
use crate::store::{TaskStore, TomlTaskStore};

// ── TaskController ────────────────────────────────────────────────────────────

/// Shared, cheaply-clonable controller for task pipeline operations.
///
/// Owns a [`TaskStore`] strategy; all callers share the same `Arc`-wrapped
/// controller instance and see a consistent view of the pipeline list.
#[derive(Clone)]
pub struct TaskController {
    store: Arc<dyn TaskStore>,
}

impl Default for TaskController {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskController {
    /// Create a controller backed by the default TOML store
    /// (`~/.config/fsn/tasks.toml`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: Arc::new(TomlTaskStore::new()),
        }
    }

    /// Create a controller with an explicit store (e.g. for tests).
    #[must_use]
    pub fn with_store(store: impl TaskStore + 'static) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    /// List all pipelines.
    pub fn list(&self) -> Vec<TaskPipeline> {
        self.store.list()
    }

    /// Retrieve a single pipeline by ID.
    pub fn get(&self, id: &str) -> Option<TaskPipeline> {
        self.store.get(id)
    }

    /// Create and persist a new pipeline.
    pub fn create(&self, name: String) -> TaskPipeline {
        self.store.create(name)
    }

    /// Delete a pipeline.  Returns `true` if it existed.
    pub fn delete(&self, id: &str) -> bool {
        self.store.delete(id)
    }

    /// Flip the `enabled` flag.
    /// Returns the new value, or `None` if the pipeline was not found.
    pub fn toggle(&self, id: &str) -> Option<bool> {
        self.store.toggle(id)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::InMemoryTaskStore;

    fn ctrl() -> TaskController {
        TaskController::with_store(InMemoryTaskStore::new())
    }

    #[test]
    fn create_and_list() {
        let c = ctrl();
        c.create("Pipeline A".into());
        assert_eq!(c.list().len(), 1);
    }

    #[test]
    fn get_existing() {
        let c = ctrl();
        let t = c.create("B".into());
        assert!(c.get(&t.id).is_some());
    }

    #[test]
    fn delete_existing() {
        let c = ctrl();
        let t = c.create("C".into());
        assert!(c.delete(&t.id));
        assert!(c.list().is_empty());
    }

    #[test]
    fn toggle_changes_enabled() {
        let c = ctrl();
        let t = c.create("D".into());
        let new_val = c.toggle(&t.id).unwrap();
        assert!(!new_val); // was true, now false
    }
}
