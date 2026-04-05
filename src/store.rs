// store.rs — TaskStore trait (Strategy Pattern).
//
// Design Pattern: Strategy — pluggable persistence for task pipelines.
//
//   TaskStore (trait)
//     ├── InMemoryTaskStore  — ephemeral; default + tests
//     └── TomlTaskStore      — persists to ~/.config/fsn/tasks.toml

use crate::model::{TaskPipeline, TasksConfig};

// ── TaskStore (Strategy) ──────────────────────────────────────────────────────

/// Pluggable persistence strategy for [`TaskPipeline`] objects.
pub trait TaskStore: Send + Sync {
    /// Return all stored pipelines.
    fn list(&self) -> Vec<TaskPipeline>;

    /// Retrieve a single pipeline by ID.
    fn get(&self, id: &str) -> Option<TaskPipeline>;

    /// Create and persist a new pipeline with the given name.
    fn create(&self, name: String) -> TaskPipeline;

    /// Remove a pipeline.  Returns `true` if it existed.
    fn delete(&self, id: &str) -> bool;

    /// Toggle the `enabled` flag.
    /// Returns the new value, or `None` if the pipeline was not found.
    fn toggle(&self, id: &str) -> Option<bool>;
}

// ── InMemoryTaskStore ─────────────────────────────────────────────────────────

/// Ephemeral in-memory store; starts empty and is lost on restart.
#[derive(Default)]
pub struct InMemoryTaskStore {
    tasks: std::sync::Mutex<Vec<TaskPipeline>>,
}

impl InMemoryTaskStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl TaskStore for InMemoryTaskStore {
    fn list(&self) -> Vec<TaskPipeline> {
        self.tasks.lock().unwrap().clone()
    }

    fn get(&self, id: &str) -> Option<TaskPipeline> {
        self.tasks
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.id == id)
            .cloned()
    }

    fn create(&self, name: String) -> TaskPipeline {
        let mut guard = self.tasks.lock().unwrap();
        #[allow(clippy::cast_possible_truncation)]
        let next_id = guard.len() as u32 + 1;
        let mut task = TaskPipeline::new_default(next_id);
        task.name = name;
        guard.push(task.clone());
        task
    }

    fn delete(&self, id: &str) -> bool {
        let mut guard = self.tasks.lock().unwrap();
        let before = guard.len();
        guard.retain(|t| t.id != id);
        guard.len() < before
    }

    fn toggle(&self, id: &str) -> Option<bool> {
        let mut guard = self.tasks.lock().unwrap();
        guard.iter_mut().find(|t| t.id == id).map(|t| {
            t.enabled = !t.enabled;
            t.enabled
        })
    }
}

// ── TomlTaskStore ─────────────────────────────────────────────────────────────

/// Durable store that loads from and saves to `~/.config/fsn/tasks.toml`.
///
/// Tasks are loaded once at construction; every mutating operation
/// persists the new state immediately.
pub struct TomlTaskStore {
    tasks: std::sync::Mutex<Vec<TaskPipeline>>,
}

impl TomlTaskStore {
    /// Load tasks from disk.  Missing or invalid files start empty.
    #[must_use]
    pub fn new() -> Self {
        let tasks = TasksConfig::load().tasks;
        Self {
            tasks: std::sync::Mutex::new(tasks),
        }
    }

    fn flush(guard: &[TaskPipeline]) {
        let cfg = TasksConfig {
            tasks: guard.to_vec(),
        };
        // Best-effort save; errors are logged but do not panic.
        if let Err(e) = cfg.save() {
            tracing::warn!("TaskStore: failed to persist tasks: {e}");
        }
    }
}

impl Default for TomlTaskStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskStore for TomlTaskStore {
    fn list(&self) -> Vec<TaskPipeline> {
        self.tasks.lock().unwrap().clone()
    }

    fn get(&self, id: &str) -> Option<TaskPipeline> {
        self.tasks
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.id == id)
            .cloned()
    }

    fn create(&self, name: String) -> TaskPipeline {
        let mut guard = self.tasks.lock().unwrap();
        #[allow(clippy::cast_possible_truncation)]
        let next_id = guard.len() as u32 + 1;
        let mut task = TaskPipeline::new_default(next_id);
        task.name = name;
        guard.push(task.clone());
        Self::flush(&guard);
        task
    }

    fn delete(&self, id: &str) -> bool {
        let mut guard = self.tasks.lock().unwrap();
        let before = guard.len();
        guard.retain(|t| t.id != id);
        let deleted = guard.len() < before;
        if deleted {
            Self::flush(&guard);
        }
        deleted
    }

    fn toggle(&self, id: &str) -> Option<bool> {
        let mut guard = self.tasks.lock().unwrap();
        let result = guard.iter_mut().find(|t| t.id == id).map(|t| {
            t.enabled = !t.enabled;
            t.enabled
        });
        if result.is_some() {
            Self::flush(&guard);
        }
        result
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> InMemoryTaskStore {
        InMemoryTaskStore::new()
    }

    #[test]
    fn create_and_list() {
        let s = store();
        let t = s.create("My Task".into());
        assert_eq!(t.name, "My Task");
        assert_eq!(s.list().len(), 1);
    }

    #[test]
    fn get_existing() {
        let s = store();
        let t = s.create("Alpha".into());
        assert!(s.get(&t.id).is_some());
    }

    #[test]
    fn get_missing_returns_none() {
        let s = store();
        assert!(s.get("no-such-id").is_none());
    }

    #[test]
    fn delete_existing() {
        let s = store();
        let t = s.create("Beta".into());
        assert!(s.delete(&t.id));
        assert!(s.list().is_empty());
    }

    #[test]
    fn delete_missing_returns_false() {
        let s = store();
        assert!(!s.delete("no-such-id"));
    }

    #[test]
    fn toggle_flips_enabled() {
        let s = store();
        let t = s.create("Gamma".into());
        assert!(t.enabled); // default is enabled
        let new_val = s.toggle(&t.id).unwrap();
        assert!(!new_val);
        let toggled_back = s.toggle(&t.id).unwrap();
        assert!(toggled_back);
    }

    #[test]
    fn toggle_missing_returns_none() {
        let s = store();
        assert!(s.toggle("no-such-id").is_none());
    }
}
