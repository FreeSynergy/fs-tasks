//! A save of the window is compare and set: a change between its reading and its writing is a conflict.
//!
//! Pattern: Decorator over the real core that lets "the command line" write at the one moment that matters.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use common::{World, rich};
use fs_components_workbenches::ErrorKind;
use fs_components_workbenches::tasks::TaskSource;
use fs_objects::ObjectId;
use fs_tasks_app::TaskAdapter;
use fs_tasks_core::{
    Capabilities, Place, Result, Revision, Task, TaskEntry, TaskItems, TaskPage, TaskQuery,
    TaskRepo,
};

/// The core, except that the first write is preceded by another client's write (`rival`).
struct Racing {
    repo: Arc<TaskRepo>,
    rival_acts: AtomicBool,
}

impl Racing {
    async fn rival(&self, id: ObjectId) {
        if self.rival_acts.swap(false, Ordering::SeqCst) {
            let mut theirs = TaskItems::get(self.repo.as_ref(), id).await.unwrap().unwrap().task;
            theirs.title = Some("Theirs".into());
            self.repo.edit(id, theirs).await.unwrap();
        }
    }
}

#[async_trait]
impl TaskItems for Racing {
    fn capabilities(&self) -> Capabilities {
        self.repo.capabilities()
    }
    async fn get(&self, id: ObjectId) -> Result<Option<TaskEntry>> {
        TaskItems::get(self.repo.as_ref(), id).await
    }
    async fn get_by_uid(&self, uid: &str) -> Result<Option<TaskEntry>> {
        self.repo.get_by_uid(uid).await
    }
    async fn list(&self) -> Result<Vec<TaskEntry>> {
        TaskItems::list(self.repo.as_ref()).await
    }
    async fn query(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>> {
        self.repo.query(query).await
    }
    async fn page(&self, q: &TaskQuery, after: Option<&Place>, size: usize) -> Result<TaskPage> {
        self.repo.page(q, after, size).await
    }
    async fn edit(&self, id: ObjectId, task: Task) -> Result<TaskEntry> {
        self.rival(id).await;
        self.repo.edit(id, task).await
    }
    async fn edit_checked(&self, id: ObjectId, expected: Revision, task: Task) -> Result<TaskEntry> {
        self.rival(id).await;
        self.repo.edit_checked(id, expected, task).await
    }
}

fn saved_after_a_rival(
    w: &World,
    key: &str,
) -> (std::result::Result<(), ErrorKind>, serde_json::Value) {
    let racing = Arc::new(Racing {
        repo: w.repo.clone(),
        rival_acts: AtomicBool::new(true),
    });
    let adapter = TaskAdapter::new(racing, w.runtime.handle().clone());
    let mut s = w.state();
    s.select(key);
    s.begin_edit();
    s.set_field("priority", "5");
    let draft = s.editing().unwrap().draft.clone();
    let got = adapter.save(key, &draft).map_err(|e| e.kind);
    (got, w.json(key))
}

#[test]
fn a_change_between_reading_and_writing_is_a_conflict_and_is_not_overwritten() {
    let w = World::new();
    let key = w.add_json(rich());
    let (got, after) = saved_after_a_rival(&w, &key);
    assert_eq!(got.unwrap_err(), ErrorKind::Conflict);
    assert_eq!(after["title"], "Theirs");
    assert_eq!(after["priority"], 2, "our write did not land");
}

#[test]
fn the_counter_test_without_a_rival_the_same_save_lands() {
    let w = World::new();
    let key = w.add_json(rich());
    let racing = Arc::new(Racing {
        repo: w.repo.clone(),
        rival_acts: AtomicBool::new(false),
    });
    let adapter = TaskAdapter::new(racing, w.runtime.handle().clone());
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.set_field("priority", "5");
    let draft = s.editing().unwrap().draft.clone();
    adapter.save(&key, &draft).unwrap();
    assert_eq!(w.json(&key)["priority"], 5);
}
