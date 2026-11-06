//! The program is usable through its traits alone: `TaskItems` as a trait object, a fake
//! implementation, and the `TaskFeed` observer (ADR 0043).

mod common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use common::{repo, t};
use fs_interfaces::Capabilities;
use fs_store::{Envelope, ObjectId};
use fs_tasks_core::{
    ImportReport, Relation, Result, Task, TaskChange, TaskEntry, TaskFeed, TaskItems, TaskListener,
    TaskQuery,
};

#[tokio::test]
async fn task_items_works_as_a_trait_object() {
    let items: Arc<dyn TaskItems> = Arc::new(repo().await);
    let mut task = Task::new("Ship");
    task.due = Some("2026-11-20T12:00:00".into());
    let e = items.create(task).await.unwrap();
    let due = items
        .query(&TaskQuery::new().due_between(t("2026-11-01T00:00:00Z"), t("2026-12-01T00:00:00Z")))
        .await
        .unwrap();
    assert_eq!((due.len(), &due[0]), (1, &e));
    assert!(
        items
            .export_ics(&[e.id])
            .await
            .unwrap()
            .contains("SUMMARY:Ship")
    );
}

/// A task store that keeps nothing: proves consumers can be tested against a fake.
struct Nothing;

#[async_trait]
impl TaskItems for Nothing {
    fn capabilities(&self) -> Capabilities {
        Capabilities::new("nothing")
    }
    async fn create(&self, task: Task) -> Result<TaskEntry> {
        Ok(TaskEntry {
            id: ObjectId::new(),
            task,
            envelope: Envelope::new("Task", "fake"),
        })
    }
    async fn edit(&self, _: ObjectId, task: Task) -> Result<TaskEntry> {
        self.create(task).await
    }
    async fn assign(&self, id: ObjectId, _: &str, _: &str) -> Result<TaskEntry> {
        self.get(id).await.map(Option::unwrap)
    }
    async fn relate(&self, id: ObjectId, _: &str, _: Relation) -> Result<TaskEntry> {
        self.get(id).await.map(Option::unwrap)
    }
    async fn label(&self, id: ObjectId, _: &str) -> Result<TaskEntry> {
        self.get(id).await.map(Option::unwrap)
    }
    async fn get(&self, id: ObjectId) -> Result<Option<TaskEntry>> {
        Ok(Some(TaskEntry {
            id,
            task: Task::new("fake"),
            envelope: Envelope::new("Task", "fake"),
        }))
    }
    async fn get_by_uid(&self, _: &str) -> Result<Option<TaskEntry>> {
        Ok(None)
    }
    async fn delete(&self, _: ObjectId) -> Result<()> {
        Ok(())
    }
    async fn list(&self) -> Result<Vec<TaskEntry>> {
        Ok(vec![])
    }
    async fn query(&self, _: &TaskQuery) -> Result<Vec<TaskEntry>> {
        Ok(vec![])
    }
    async fn import_ics(&self, _: &str) -> Result<ImportReport> {
        Ok(ImportReport::default())
    }
    async fn export_ics(&self, _: &[ObjectId]) -> Result<String> {
        Ok(String::new())
    }
}

#[tokio::test]
async fn a_fake_implements_the_interface() {
    let items: Arc<dyn TaskItems> = Arc::new(Nothing);
    assert_eq!(items.list().await.unwrap().len(), 0);
    assert_eq!(items.capabilities().provider(), "nothing");
}

#[derive(Default)]
struct Log(Mutex<Vec<TaskChange>>);

impl TaskListener for Log {
    fn changed(&self, change: &TaskChange) {
        self.0.lock().unwrap().push(*change);
    }
}

#[tokio::test]
async fn subscribers_see_changes_until_dropped() {
    let r = repo().await;
    let log = Arc::new(Log::default());
    let feed: &dyn TaskFeed = &r;
    let sub = feed.subscribe(log.clone());
    let e = r.create(Task::new("Watched")).await.unwrap();
    r.label(e.id, "x").await.unwrap();
    r.delete(e.id).await.unwrap();
    assert_eq!(
        *log.0.lock().unwrap(),
        [
            TaskChange::Created(e.id),
            TaskChange::Updated(e.id),
            TaskChange::Deleted(e.id)
        ]
    );
    drop(sub);
    r.create(Task::new("Unwatched")).await.unwrap();
    assert_eq!(
        log.0.lock().unwrap().len(),
        3,
        "no change after unsubscribing"
    );
}
