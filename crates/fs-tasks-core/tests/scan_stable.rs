//! A scan goes on from the last id it saw, not by a count of rows: an edit made between two of its queries (which moves
//! the task in the order of changes) neither hides a task nor shows one twice.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use common::ME;
use fs_objects::traits::{ObjectQuery, ObjectRepository};
use fs_objects::{Query, Store, StoredObject, TursoStore};
use fs_tasks_core::{Task, TaskItems, TaskQuery, TaskRepo};

/// The store as a query backend that, at its third query, edits the task `t0` (the oldest, so it moves to the front of
/// the order of changes while the scan is half way).
struct Editing {
    store: Arc<dyn Store>,
    calls: AtomicUsize,
    /// What it does at the third query: edit `t0`, or create a task whose id is below every id the scan has seen.
    create_small_id: bool,
}

#[async_trait]
impl ObjectQuery for Editing {
    async fn query(&self, q: &Query) -> fs_objects::Result<Vec<StoredObject>> {
        let found = self.store.query(q).await?;
        if self.calls.fetch_add(1, Ordering::SeqCst) == 2 {
            let repo = TaskRepo::new(self.store.clone(), ME);
            if self.create_small_id {
                let mut t = Task::new("early");
                t.uid = "urn:uuid:00000000-0000-7000-8000-000000000001".into();
                repo.create(t).await.unwrap();
                return Ok(found);
            }
            let t0 = repo.get_by_uid("t0").await.unwrap().unwrap();
            let mut t = t0.task.clone();
            t.title = Some("edited during the scan".into());
            repo.edit(t0.id, t).await.unwrap();
        }
        Ok(found)
    }
    async fn count(&self, q: &Query) -> fs_objects::Result<usize> {
        self.store.count(q).await
    }
}

#[tokio::test]
async fn an_edit_during_the_scan_changes_nothing_about_who_is_in_it() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let plain = TaskRepo::new(store.clone(), ME);
    for i in 0..30 {
        let mut t = Task::new(format!("t{i}"));
        t.uid = format!("t{i}");
        plain.create(t).await.unwrap();
    }
    // objects and queries go to the store; only the queries are watched
    let objects: Arc<dyn ObjectRepository> = Arc::new(StoreObjects(store.clone()));
    let watched = TaskRepo::with_backend(
        objects,
        Arc::new(Editing {
            store: store.clone(),
            calls: AtomicUsize::new(0),
            create_small_id: false,
        }),
        ME,
    )
    .with_scan_chunk(7);
    // every task is ready (no relations): the page walks the scan, which reads 7 at a time
    let page = watched
        .page(&TaskQuery::new().only_ready(true), None, 100)
        .await
        .unwrap();
    let mut uids: Vec<String> = page
        .items
        .iter()
        .map(|p| p.entry.task.uid.clone())
        .collect();
    assert_eq!(uids.len(), 30, "no task hidden");
    uids.sort();
    uids.dedup();
    assert_eq!(uids.len(), 30, "no task twice");
}

/// A task that appears during the scan with an id below the scan's position is no error (the scan once called that
/// "tasks without an id key"); it is stored later than the others, so it comes at the end of the scan.
#[tokio::test]
async fn a_task_created_during_the_scan_with_a_smaller_id_is_no_error() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let plain = TaskRepo::new(store.clone(), ME);
    for i in 0..30 {
        let mut t = Task::new(format!("t{i}"));
        t.uid = format!("t{i}");
        plain.create(t).await.unwrap();
    }
    let objects: Arc<dyn ObjectRepository> = Arc::new(StoreObjects(store.clone()));
    let watched = TaskRepo::with_backend(
        objects,
        Arc::new(Editing {
            store,
            calls: AtomicUsize::new(0),
            create_small_id: true,
        }),
        ME,
    )
    .with_scan_chunk(7);
    let page = watched
        .page(&TaskQuery::new().only_ready(true), None, 100)
        .await
        .expect("a task created during the scan is not an error");
    assert_eq!(
        page.items.len(),
        31,
        "the 30 that were there, and the new one at the end of the scan"
    );
}

struct StoreObjects(Arc<dyn Store>);

#[async_trait]
impl ObjectRepository for StoreObjects {
    async fn put(&self, o: StoredObject, actor: &str) -> fs_objects::Result<fs_objects::Activity> {
        self.0.put(o, actor).await
    }
    async fn put_and_log(
        &self,
        o: StoredObject,
        actor: &str,
        extra: Vec<fs_objects::LogEntry>,
    ) -> fs_objects::Result<Vec<fs_objects::Activity>> {
        self.0.put_and_log(o, actor, extra).await
    }
    async fn get(&self, id: fs_objects::ObjectId) -> fs_objects::Result<Option<StoredObject>> {
        self.0.get(id).await
    }
    async fn delete(
        &self,
        id: fs_objects::ObjectId,
        actor: &str,
    ) -> fs_objects::Result<fs_objects::Activity> {
        self.0.delete(id, actor).await
    }
}
