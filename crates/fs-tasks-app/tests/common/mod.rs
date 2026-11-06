//! Shared helpers of the tests: the real core over an in-memory store, behind the real adapter.
//!
//! Pattern: Facade over the set-up. Nothing here pretends to be the core.
#![allow(dead_code)] // each test file uses a different subset

use std::path::PathBuf;
use std::sync::Arc;

use fs_components_workbenches::tasks::{TaskLabels, TaskSource, TasksState, Variant};
use fs_store::{ObjectId, Store, StoredObject, TursoStore};
use fs_tasks_app::TaskAdapter;
use fs_tasks_core::{Task, TaskItems, TaskRepo};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

pub const ME: &str = "https://local.invalid/users/owner";

/// A runtime, a repository over a store, and the adapter over it.
pub struct World {
    pub runtime: Runtime,
    pub store: Arc<dyn Store>,
    pub repo: Arc<TaskRepo>,
    pub adapter: Arc<TaskAdapter>,
}

impl World {
    pub fn new() -> Self {
        let runtime = Runtime::new().unwrap();
        let store: Arc<dyn Store> =
            Arc::new(runtime.block_on(TursoStore::open_in_memory()).unwrap());
        let repo = Arc::new(TaskRepo::new(store.clone(), ME));
        let adapter = Arc::new(TaskAdapter::new(repo.clone(), runtime.handle().clone()));
        Self {
            runtime,
            store,
            repo,
            adapter,
        }
    }

    /// Store `task` through the core; returns the key.
    pub fn add(&self, task: Task) -> String {
        self.runtime
            .block_on(self.repo.create(task))
            .unwrap()
            .id
            .to_string()
    }

    /// Store JSON through the core.
    pub fn add_json(&self, value: Value) -> String {
        self.add(Task::from_value(value).unwrap())
    }

    /// Store a plain task with this title.
    pub fn plain(&self, title: &str) -> String {
        self.add(Task::new(title))
    }

    pub fn stored(&self, key: &str) -> StoredObject {
        let id: ObjectId = key.parse().unwrap();
        self.runtime.block_on(self.store.get(id)).unwrap().unwrap()
    }

    /// The task as stored, as JSON.
    pub fn json(&self, key: &str) -> Value {
        self.stored(key).content
    }

    /// The state machine over the real core.
    pub fn state(&self) -> TasksState {
        let source: Arc<dyn TaskSource> = self.adapter.clone();
        TasksState::new(source, TaskLabels::default(), Variant::Full)
    }

    /// The core edits this task behind the workbench's back.
    pub fn other_changes(&self, key: &str, title: &str) {
        let id: ObjectId = key.parse().unwrap();
        let mut entry = self
            .runtime
            .block_on(TaskItems::get(self.repo.as_ref(), id))
            .unwrap()
            .unwrap();
        entry.task.title = Some(title.into());
        self.runtime
            .block_on(self.repo.edit(id, entry.task))
            .unwrap();
    }

    /// Mark the stored object on its envelope, as a store can hold it.
    pub fn mark_envelope(&self, key: &str) {
        let mut object = self.stored(key);
        object.envelope.sensitive = vec!["pd:HealthRecord".into()];
        self.runtime.block_on(self.store.put(object, ME)).unwrap();
    }
}

/// `value` without the members a save always sets, and those named.
pub fn without(mut value: Value, names: &[&str]) -> Value {
    let members = value.as_object_mut().unwrap();
    members.remove("updated");
    for name in names {
        members.remove(*name);
    }
    value
}

/// A task with everything a form does not show.
pub fn rich() -> Value {
    json!({
        "@type": "Task",
        "version": "2.0",
        "uid": "urn:uuid:019a0000-0000-7000-8000-00000000aaaa",
        "prodId": "-//other//program//EN",
        "created": "2026-01-01T10:00:00Z",
        "updated": "2026-01-01T10:00:00Z",
        "sequence": 3,
        "title": "Rich",
        "description": "Line one\nLine two",
        "descriptionContentType": "text/plain",
        "locale": "de",
        "start": "2026-11-01T09:00:00",
        "timeZone": "Europe/Berlin",
        "due": "2026-11-12T17:00:30",
        "estimatedDuration": "PT2H",
        "percentComplete": 40,
        "progress": "in-process",
        "progressUpdated": "2026-02-02T10:00:00Z",
        "priority": 2,
        "keywords": {"one": true, "off": false, "with,comma": true},
        "categories": {"http://example.com/x": true},
        "participants": {
            "p1": {"@type": "Participant", "name": "Ada", "email": "ada@example.com",
                   "roles": {"owner": true}, "progress": "in-process", "x-other": {"a": [1, 2]}}
        },
        "relatedTo": {
            "urn:uuid:019a0000-0000-7000-8000-00000000bbbb":
                {"@type": "Relation", "relation": {"depends-on": true}, "x-keep": 1}
        },
        "alerts": {"a1": {"@type": "Alert", "trigger": {"@type": "OffsetTrigger", "offset": "-PT15M"}}},
        "links": {"l1": {"@type": "Link", "href": "https://example.com/a"}},
        "freesynergy.net:taskStatus": "pending",
        "example.com:vendor": {"nested": [true, null, 1.5]}
    })
}

/// A task that repeats, with an override.
pub fn series() -> Value {
    json!({
        "@type": "Task",
        "version": "2.0",
        "uid": "urn:uuid:019a0000-0000-7000-8000-00000000cccc",
        "title": "Weekly report",
        "start": "2026-11-02T09:00:00",
        "timeZone": "Europe/Berlin",
        "due": "2026-11-02T17:00:00",
        "progress": "needs-action",
        "recurrenceRule": {"@type": "RecurrenceRule", "frequency": "weekly"},
        "recurrenceOverrides": {"2026-11-09T09:00:00": {"title": "Longer report"}}
    })
}

/// A folder for the files of one test, removed when the test ends (also when it fails).
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("fs-tasks-app-{}", ObjectId::new()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
