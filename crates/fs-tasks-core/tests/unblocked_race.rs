#![allow(clippy::pedantic)]
//! Two writers: a completion is judged while another writer reopens the task. "After" is the store as it is when the
//! change is judged, so the later writer decides and no `Unblocked` is announced for a task that waits again. And a call
//! that queued a change returns only after the change was judged: its own announcement is out when it returns.
mod common;
use async_trait::async_trait;
use common::ME;
use fs_objects::traits::{ObjectQuery, ObjectRepository};
use fs_objects::{Query, Store, StoredObject, TursoStore};
use fs_tasks_core::plan::Readiness;
use fs_tasks_core::{Relation, Task, TaskChange, TaskFeed, TaskItems, TaskListener, TaskRepo};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
struct Log(Mutex<Vec<TaskChange>>);
impl TaskListener for Log {
    fn changed(&self, c: &TaskChange) {
        self.0.lock().unwrap().push(*c);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reopen_during_the_judgement_of_a_completion_announces_nothing() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let fast = TaskRepo::new(store.clone(), ME);
    let mut x = Task::new("x");
    x.uid = "x".into();
    x.relate("z", Relation::FinishToStart);
    let x = fast.create(x).await.unwrap();
    let mut z = Task::new("z");
    z.uid = "z".into();
    let z = fast.create(z).await.unwrap();
    assert_ne!(
        fast.readiness(z.id).await.unwrap(),
        Readiness::Ready,
        "z waits for x"
    );
    let slow = TaskRepo::with_backend(
        Arc::new(Plain(store.clone())),
        Arc::new(Slow(store, Duration::from_millis(60))),
        ME,
    );
    let log = Arc::new(Log(Mutex::new(Vec::new())));
    let _sub = slow.subscribe(log.clone());
    let (a, b) = (slow.clone(), slow.clone());
    let mut done = x.task.clone();
    done.progress = Some("completed".into());
    let ha = tokio::spawn(async move {
        a.edit(x.id, done).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(5)).await;
    let mut open = x.task.clone();
    open.progress = Some("needs-action".into());
    let hb = tokio::spawn(async move {
        b.edit(x.id, open).await.unwrap();
    });
    ha.await.unwrap();
    hb.await.unwrap();
    let events = log.0.lock().unwrap().clone();
    let named: Vec<String> = events
        .iter()
        .map(|e| match e {
            TaskChange::Updated(i) if *i == x.id => "Updated(x)".into(),
            TaskChange::Unblocked(i) if *i == z.id => "Unblocked(z)".into(),
            o => format!("{o:?}"),
        })
        .collect();
    let end = slow.readiness(z.id).await.unwrap();
    eprintln!("REVIEW events: {named:?}; z at the end: {end:?}");
    let last_unblocked = named.iter().rposition(|n| n == "Unblocked(z)");
    let last_update = named.iter().rposition(|n| n == "Updated(x)");
    assert_ne!(end, Readiness::Ready, "x was reopened last, z waits again");
    assert!(
        last_unblocked.is_none() || last_unblocked < last_update,
        "Unblocked(z) is the last word although z is not ready: {named:?}"
    );
}

/// Two writers each complete the task that another one waits for; when a call returns, its own `Unblocked` is out.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_call_returns_only_after_its_own_announcement_was_sent() {
    for _round in 0..8 {
        let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
        let fast = TaskRepo::new(store.clone(), ME);
        let mut firsts = Vec::new();
        let mut seconds = Vec::new();
        for i in 0..2 {
            let mut x = Task::new(format!("x{i}"));
            x.uid = format!("x{i}");
            x.relate(&format!("z{i}"), Relation::FinishToStart);
            firsts.push(fast.create(x).await.unwrap());
            let mut z = Task::new(format!("z{i}"));
            z.uid = format!("z{i}");
            seconds.push(fast.create(z).await.unwrap());
        }
        let slow = TaskRepo::with_backend(
            Arc::new(Plain(store.clone())),
            Arc::new(Slow(store, Duration::from_millis(10))),
            ME,
        );
        let log = Arc::new(Log(Mutex::new(Vec::new())));
        let _sub = slow.subscribe(log.clone());
        let mut handles = Vec::new();
        for i in 0..2 {
            let repo = slow.clone();
            let log = log.clone();
            let (x, z) = (firsts[i].clone(), seconds[i].id);
            handles.push(tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(3 * i as u64)).await;
                let mut done = x.task.clone();
                done.progress = Some("completed".into());
                repo.edit(x.id, done).await.unwrap();
                let seen = log.0.lock().unwrap().contains(&TaskChange::Unblocked(z));
                assert!(seen, "the call returned before its own Unblocked was sent");
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
    }
}
