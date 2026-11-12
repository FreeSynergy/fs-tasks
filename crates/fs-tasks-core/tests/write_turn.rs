//! Who became ready is judged after the write turn is given back: another write does not wait for the (possibly long)
//! reading of a deep graph. The queries are slowed down, so the order of the two finishing is the proof.

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use common::ME;
use fs_objects::traits::{ObjectQuery, ObjectRepository};
use fs_objects::{Query, Store, StoredObject, TursoStore};
use fs_tasks_core::{Relation, Task, TaskItems, TaskRepo};

struct Slow(Arc<dyn Store>, Duration);

#[async_trait]
impl ObjectQuery for Slow {
    async fn query(&self, q: &Query) -> fs_objects::Result<Vec<StoredObject>> {
        tokio::time::sleep(self.1).await;
        self.0.query(q).await
    }
    async fn count(&self, q: &Query) -> fs_objects::Result<usize> {
        self.0.count(q).await
    }
}

struct Plain(Arc<dyn Store>);

#[async_trait]
impl ObjectRepository for Plain {
    async fn put(&self, o: StoredObject, a: &str) -> fs_objects::Result<fs_objects::Activity> {
        self.0.put(o, a).await
    }
    async fn put_and_log(
        &self,
        o: StoredObject,
        a: &str,
        e: Vec<fs_objects::LogEntry>,
    ) -> fs_objects::Result<Vec<fs_objects::Activity>> {
        self.0.put_and_log(o, a, e).await
    }
    async fn get(&self, id: fs_objects::ObjectId) -> fs_objects::Result<Option<StoredObject>> {
        self.0.get(id).await
    }
    async fn delete(
        &self,
        id: fs_objects::ObjectId,
        a: &str,
    ) -> fs_objects::Result<fs_objects::Activity> {
        self.0.delete(id, a).await
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_write_does_not_wait_for_the_judgement_of_a_change() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let fast = TaskRepo::new(store.clone(), ME);
    let mut head_id = None;
    for i in 0..6 {
        let mut t = Task::new(format!("t{i}"));
        t.uid = format!("t{i}");
        if i + 1 < 6 {
            t.relate(&format!("t{}", i + 1), Relation::FinishToStart);
        }
        let id = fast.create(t).await.unwrap().id;
        head_id.get_or_insert(id);
    }
    let slow = TaskRepo::with_backend(
        Arc::new(Plain(store.clone())),
        Arc::new(Slow(store, Duration::from_millis(60))),
        ME,
    );
    let head = slow.get(head_id.unwrap()).await.unwrap().unwrap();
    let started = Instant::now();
    let finishing = slow.clone();
    let completing = tokio::spawn(async move {
        let mut t = head.task.clone();
        t.progress = Some("completed".into());
        finishing.edit(head.id, t).await.unwrap();
        started.elapsed()
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    let mut other = Task::new("other");
    other.uid = "other".into();
    slow.create(other).await.unwrap();
    let other_done = started.elapsed();
    let edit_done = completing.await.unwrap();
    assert!(
        other_done < edit_done,
        "the create ({other_done:?}) waited for the whole edit ({edit_done:?}) including its judgement"
    );
}

/// Counts the queries made, so a judgement (which reads the graph) shows.
struct Counting(Arc<dyn Store>, Arc<std::sync::atomic::AtomicUsize>);
#[async_trait]
impl ObjectQuery for Counting {
    async fn query(&self, q: &Query) -> fs_objects::Result<Vec<StoredObject>> {
        self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.0.query(q).await
    }
    async fn count(&self, q: &Query) -> fs_objects::Result<usize> {
        self.0.count(q).await
    }
}

/// Taking a relation away can free a task, so it is judged; adding one, changing its gap or a title cannot, so none of
/// those reads the graph (a new task in front of a long chain is not made to scan it).
#[tokio::test]
async fn only_a_change_that_can_free_somebody_is_judged() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let plain = TaskRepo::new(store.clone(), ME);
    let mut ids = Vec::new();
    for i in 0..4 {
        let mut t = Task::new(format!("t{i}"));
        t.uid = format!("t{i}");
        ids.push(plain.create(t).await.unwrap().id);
    }
    // count the queries of each kind of write: a judgement adds many
    let queries = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let repo = TaskRepo::with_backend(
        Arc::new(Plain(store.clone())),
        Arc::new(Counting(store, queries.clone())),
        ME,
    );
    let used = |f: &std::sync::atomic::AtomicUsize| f.swap(0, std::sync::atomic::Ordering::SeqCst);
    // adding a relation: the check for a cycle asks a few things, nothing is judged
    repo.relate(ids[0], "t1", Relation::FinishToStart)
        .await
        .unwrap();
    let adding = used(&queries);
    // the same relation with a gap: only the gap changes
    repo.relate_with_gap(ids[0], "t1", Relation::FinishToStart, "P1D")
        .await
        .unwrap();
    let gap = used(&queries);
    // a title
    let mut t = repo.get(ids[0]).await.unwrap().unwrap().task;
    t.title = Some("other".into());
    repo.edit(ids[0], t).await.unwrap();
    let title = used(&queries);
    // taking the relation away: judged
    let mut t = repo.get(ids[0]).await.unwrap().unwrap().task;
    t.related_to.clear();
    used(&queries);
    repo.edit(ids[0], t).await.unwrap();
    let removing = used(&queries);
    assert!(
        removing > adding.max(gap).max(title),
        "adding {adding}, gap {gap}, title {title}, removing {removing}"
    );
}
