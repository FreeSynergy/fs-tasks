//! What must hold whatever a client sends: nothing overwritten, nothing lost, no crash.

mod common;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use common::{repo, t};
use fs_store::traits::{ObjectQuery, ObjectRepository};
use fs_store::{
    Activity, LogEntry, ObjectId, Query, Result as StoreResult, Store, StoredObject, TursoStore,
};
use fs_tasks_core::{Error, Task, TaskItems, TaskQuery, TaskRepo};

fn task(uid: &str, json_due: Option<&str>) -> Task {
    let mut v = serde_json::json!({"@type": "Task", "version": "2.0", "uid": uid, "title": uid});
    if let Some(due) = json_due {
        v["due"] = due.into();
    }
    Task::from_value(v).unwrap()
}

const UUID: &str = "019a0000-0000-7000-8000-000000000001";

#[tokio::test]
async fn the_same_uuid_in_another_form_does_not_overwrite() {
    let r = repo().await;
    r.create(task(&format!("urn:uuid:{UUID}"), None))
        .await
        .unwrap();
    let second = r.create(task(UUID, None)).await;
    assert!(matches!(second, Err(Error::Conflict(_))), "{second:?}");
    assert_eq!(r.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn the_id_of_another_task_as_uid_does_not_overwrite() {
    let r = repo().await;
    let first = r.create(task("plain-uid", None)).await.unwrap();
    let second = r.create(task(&first.id.to_string(), None)).await;
    assert!(matches!(second, Err(Error::Conflict(_))), "{second:?}");
    assert!(r.get_by_uid("plain-uid").await.unwrap().is_some());
}

#[tokio::test]
async fn an_absurd_duration_is_refused_not_a_crash() {
    let r = repo().await;
    let v = serde_json::json!({"@type": "Task", "version": "2.0", "uid": "x", "title": "x",
        "start": "2026-01-01T00:00:00", "estimatedDuration": "P99999999999W"});
    assert!(r.create(Task::from_value(v).unwrap()).await.is_err());
}

#[tokio::test]
async fn a_range_respects_fractions_of_a_second() {
    let r = repo().await;
    r.create(task("due", Some("2026-12-01T10:00:00")))
        .await
        .unwrap();
    let q = |from: &str, to: &str| TaskQuery::new().due_between(t(from), t(to));
    let hits = async |q: TaskQuery| r.query(&q).await.unwrap().len();
    let due = "2026-12-01T10:00:00Z";
    assert_eq!(
        hits(q("2026-12-01T09:00:00Z", due)).await,
        0,
        "the end is excluded"
    );
    assert_eq!(
        hits(q(due, "2026-12-01T11:00:00Z")).await,
        1,
        "the start is included"
    );
    let half = "2026-12-01T10:00:00.500Z".parse().unwrap();
    let mut before_half = q("2026-12-01T09:00:00Z", due);
    before_half.due_to = Some(half);
    assert_eq!(hits(before_half).await, 1, "due before the half second");
    let mut after_half = q(due, "2026-12-01T11:00:00Z");
    after_half.due_from = Some(half);
    assert_eq!(hits(after_half).await, 0, "due before the half second");
    let mut far = q(due, due);
    far.due_to = Some("+12000-01-01T00:00:00Z".parse().unwrap());
    assert_eq!(hits(far).await, 1, "a bound after year 9999 still sorts");
}

#[tokio::test]
async fn progress_filters_find_the_computed_default() {
    let r = repo().await;
    let mut done = task("done", None);
    done.assign("A", "a@example.org");
    for p in done.participants.values_mut() {
        p.extra.insert("progress".into(), "completed".into());
    }
    r.create(done).await.unwrap();
    r.create(task("open", None)).await.unwrap();
    let uids = async |p: &str| {
        let q = TaskQuery::new().with_progress(p);
        let all = r.query(&q).await.unwrap();
        all.into_iter().map(|e| e.task.uid).collect::<Vec<_>>()
    };
    assert_eq!(uids("needs-action").await, ["open"]);
    assert_eq!(uids("completed").await, ["done"]);
}

#[tokio::test]
async fn the_list_is_by_due_date_with_undated_last() {
    let r = repo().await;
    let v = serde_json::json!({"@type": "Task", "version": "2.0", "uid": "start-only",
        "title": "s", "start": "2026-01-01T00:00:00"});
    r.create(Task::from_value(v).unwrap()).await.unwrap();
    r.create(task("march", Some("2026-03-01T00:00:00")))
        .await
        .unwrap();
    r.create(task("feb", Some("2026-02-01T00:00:00")))
        .await
        .unwrap();
    let order: Vec<_> = r
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.task.uid)
        .collect();
    assert_eq!(order, ["feb", "march", "start-only"]);
}

#[tokio::test]
async fn an_edit_takes_what_a_create_takes() {
    let r = repo().await;
    let bare = || serde_json::json!({"uid": "bare", "title": "t"});
    let first = r.create(Task::from_value(bare()).unwrap()).await.unwrap();
    r.edit(first.id, Task::from_value(bare()).unwrap())
        .await
        .unwrap();
}

#[tokio::test]
async fn small_edits_cannot_grow_a_task_past_what_fits() {
    let r = repo().await;
    let id = r.create(task("grows", None)).await.unwrap().id;
    let big = "x".repeat(6 * 1024 * 1024);
    let mut last = Ok(());
    for i in 0..4 {
        last = r.label(id, &format!("{i}{big}")).await.map(|_| ());
    }
    assert!(matches!(last, Err(Error::TooLarge(_))), "{last:?}");
    assert!(r.get(id).await.unwrap().is_some());
    let escaped = serde_json::json!({"@type": "Task", "version": "2.0", "uid": "e",
        "title": "\u{1}".repeat(3 * 1024 * 1024)});
    let refused = r.create(Task::from_value(escaped).unwrap()).await;
    assert!(
        matches!(refused, Err(Error::TooLarge(_))),
        "escaping counts"
    );
}

#[tokio::test]
async fn extreme_query_bounds_answer_and_the_last_second_counts() {
    let r = repo().await;
    r.create(task("last", Some("9999-12-31T23:59:59")))
        .await
        .unwrap();
    let extreme = chrono::DateTime::from_timestamp_millis(8_210_266_876_799_999).unwrap();
    let mut q = TaskQuery::new();
    q.due_to = Some(extreme);
    assert_eq!(
        r.query(&q).await.unwrap().len(),
        1,
        "no panic, all before it"
    );
    let mut q = TaskQuery::new();
    q.due_from = Some(t("9999-01-01T00:00:00Z"));
    assert_eq!(
        r.query(&q).await.unwrap().len(),
        1,
        "a from without a to is open"
    );
}

/// A backend that yields at every step, as a remote one would.
struct Yielding(Arc<dyn Store>);

async fn pause() {
    tokio::time::sleep(Duration::from_millis(1)).await;
}

#[async_trait]
impl ObjectRepository for Yielding {
    async fn put(&self, object: StoredObject, actor: &str) -> StoreResult<Activity> {
        pause().await;
        self.0.put(object, actor).await
    }
    async fn put_and_log(
        &self,
        object: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> StoreResult<Vec<Activity>> {
        pause().await;
        self.0.put_and_log(object, actor, extra).await
    }
    async fn get(&self, id: ObjectId) -> StoreResult<Option<StoredObject>> {
        pause().await;
        self.0.get(id).await
    }
    async fn delete(&self, id: ObjectId, actor: &str) -> StoreResult<Activity> {
        pause().await;
        self.0.delete(id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for Yielding {
    async fn query(&self, query: &Query) -> StoreResult<Vec<StoredObject>> {
        pause().await;
        self.0.query(query).await
    }
    async fn count(&self, query: &Query) -> StoreResult<usize> {
        pause().await;
        self.0.count(query).await
    }
}

#[tokio::test]
async fn concurrent_writes_lose_nothing_even_on_a_backend_that_yields() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let backend = Arc::new(Yielding(store));
    let r = TaskRepo::with_backend(backend.clone(), backend, common::ME);
    let id = r.create(task("shared", None)).await.unwrap().id;
    let labels = (0..20).map(|i| {
        let r = r.clone();
        async move { r.label(id, &format!("l{i}")).await.unwrap() }
    });
    futures_join_all(labels).await;
    assert_eq!(r.get(id).await.unwrap().unwrap().task.keywords.len(), 20);

    let creates = (0..5).map(|_| {
        let r = r.clone();
        async move { r.create(task("same", None)).await.is_ok() }
    });
    let won = futures_join_all(creates).await;
    assert_eq!(won.iter().filter(|ok| **ok).count(), 1);
}

async fn futures_join_all<F: Future + Send + 'static>(
    all: impl Iterator<Item = F>,
) -> Vec<F::Output>
where
    F::Output: Send + 'static,
{
    let handles: Vec<_> = all.map(tokio::spawn).collect();
    let mut out = Vec::new();
    for handle in handles {
        out.push(handle.await.unwrap());
    }
    out
}
