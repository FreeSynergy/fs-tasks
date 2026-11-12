//! `Unblocked`: who becomes ready because a task started, was completed, lost a relation or was deleted.

mod common;

use std::sync::{Arc, Mutex};

use common::repo;
use fs_tasks_core::{
    Relation, Task, TaskChange, TaskEntry, TaskFeed, TaskItems, TaskListener, TaskRepo,
};

struct Log(Mutex<Vec<TaskChange>>);
impl TaskListener for Log {
    fn changed(&self, change: &TaskChange) {
        self.0.lock().unwrap().push(*change);
    }
}

async fn make(r: &TaskRepo, uid: &str, progress: &str) -> TaskEntry {
    let mut t = Task::new(uid);
    t.uid = uid.into();
    t.progress = Some(progress.into());
    r.create(t).await.unwrap()
}

fn unblocked(log: &Log) -> Vec<fs_objects::ObjectId> {
    log.0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|c| match c {
            TaskChange::Unblocked(id) => Some(*id),
            _ => None,
        })
        .collect()
}

async fn set_progress(r: &TaskRepo, e: &TaskEntry, progress: &str) {
    let mut t = r.get(e.id).await.unwrap().unwrap().task;
    t.progress = Some(progress.into());
    r.edit(e.id, t).await.unwrap();
}

#[tokio::test]
async fn a_start_to_start_successor_is_announced_when_the_other_starts_not_before() {
    let r = repo().await;
    let a = make(&r, "a", "needs-action").await;
    let b = make(&r, "b", "needs-action").await;
    r.relate(a.id, "b", Relation::StartToStart).await.unwrap();
    let log = Arc::new(Log(Mutex::default()));
    let _sub = r.subscribe(log.clone());
    set_progress(&r, &a, "in-process").await;
    assert_eq!(unblocked(&log), [b.id]);
    // completing it later frees nothing new: the tie was satisfied already
    set_progress(&r, &a, "completed").await;
    assert_eq!(unblocked(&log), [b.id], "announced once");
}

#[tokio::test]
async fn a_finish_to_start_successor_is_not_announced_when_the_other_only_starts() {
    let r = repo().await;
    let a = make(&r, "a", "needs-action").await;
    let b = make(&r, "b", "needs-action").await;
    r.relate(a.id, "b", Relation::FinishToStart).await.unwrap();
    let log = Arc::new(Log(Mutex::default()));
    let _sub = r.subscribe(log.clone());
    set_progress(&r, &a, "in-process").await;
    assert_eq!(unblocked(&log), []);
    set_progress(&r, &a, "completed").await;
    assert_eq!(unblocked(&log), [b.id]);
}

#[tokio::test]
async fn removing_the_relation_frees_the_waiting_task() {
    let r = repo().await;
    let a = make(&r, "a", "needs-action").await;
    let b = make(&r, "b", "needs-action").await;
    r.relate(a.id, "b", Relation::FinishToStart).await.unwrap();
    let log = Arc::new(Log(Mutex::default()));
    let _sub = r.subscribe(log.clone());
    // `a` gives up the relation it held: `b` waits for nothing now
    let mut changed = r.get(a.id).await.unwrap().unwrap().task;
    changed.related_to.clear();
    r.edit(a.id, changed).await.unwrap();
    assert_eq!(unblocked(&log), [b.id]);
    // `b` gives up a dependency it held itself: it announces itself
    let c = make(&r, "c", "needs-action").await;
    r.relate(b.id, "c", Relation::DependsOn).await.unwrap();
    let before = unblocked(&log).len();
    let mut free = r.get(b.id).await.unwrap().unwrap().task;
    free.related_to.clear();
    r.edit(b.id, free).await.unwrap();
    assert_eq!(unblocked(&log).len(), before + 1);
    assert_eq!(*unblocked(&log).last().unwrap(), b.id);
    let _ = c;
}

#[tokio::test]
async fn deleting_a_blocker_frees_the_task_that_waited_for_it() {
    let r = repo().await;
    let a = make(&r, "a", "needs-action").await;
    let b = make(&r, "b", "needs-action").await;
    let other = make(&r, "other", "needs-action").await;
    r.relate(a.id, "b", Relation::FinishToStart).await.unwrap();
    r.relate(other.id, "b", Relation::FinishToStart)
        .await
        .unwrap();
    let log = Arc::new(Log(Mutex::default()));
    let _sub = r.subscribe(log.clone());
    r.delete(a.id).await.unwrap();
    assert!(unblocked(&log).is_empty(), "`other` still blocks `b`");
    r.delete(other.id).await.unwrap();
    assert_eq!(unblocked(&log), [b.id]);
}

#[tokio::test]
async fn readiness_of_many_matches_the_one_by_one_answers() {
    let r = repo().await;
    let a = make(&r, "a", "needs-action").await;
    let b = make(&r, "b", "needs-action").await;
    let c = make(&r, "c", "completed").await;
    r.relate(a.id, "b", Relation::FinishToStart).await.unwrap();
    let many = r.readiness_many(&[a.id, b.id, c.id]).await.unwrap();
    for (id, readiness) in &many {
        assert_eq!(*readiness, r.readiness(*id).await.unwrap());
    }
    assert_eq!(many.len(), 3);
    assert!(
        r.readiness_many(&[fs_objects::ObjectId::new()])
            .await
            .is_err()
    );
}
