//! Task queries: due dates, ranges, status, labels and ordering in the store.

mod common;

use common::{repo, setup, t};
use fs_tasks_core::{Task, TaskItems, TaskQuery};
use serde_json::json;

#[tokio::test]
async fn tasks_by_due_date() {
    let (r, store) = setup().await;
    let mut a = Task::new("Write the minutes");
    a.due = Some("2026-11-12T17:00:00".into());
    a.time_zone = Some("Europe/Berlin".into());
    a.set_vendor("freesynergy.net:taskStatus", json!("waiting-for-input"));
    let oa = r.create(a).await.unwrap();
    r.create(Task::new("Someday")).await.unwrap();
    let stored = store.get(oa.id).await.unwrap().unwrap();
    assert_eq!(stored.envelope.object_type, "Task");
    assert_eq!(stored.index["due_utc"], ["2026-11-12T16:00:00Z"]);
    assert_eq!(stored.index["progress"], ["needs-action"]);
    assert_eq!(stored.index["task_status"], ["waiting-for-input"]);
    let q = TaskQuery::new().due_between(t("2026-11-01T00:00:00Z"), t("2026-12-01T00:00:00Z"));
    assert_eq!(r.query(&q).await.unwrap().len(), 1);
    let all = r.list().await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(
        all[0].task.title.as_deref(),
        Some("Write the minutes"),
        "undated last"
    );
    let mut task = r.get(oa.id).await.unwrap().unwrap().task;
    assert_eq!(task.task_status(), Some("waiting-for-input"));
    task.progress = Some("completed".into());
    task.percent_complete = Some(100);
    r.edit(oa.id, task).await.unwrap();
    assert_eq!(
        r.get(oa.id)
            .await
            .unwrap()
            .unwrap()
            .task
            .progress
            .as_deref(),
        Some("completed")
    );
}

#[tokio::test]
async fn task_ranges_and_ordering_in_the_store() {
    let r = repo().await;
    for (title, due) in [
        ("c", Some("2026-11-12T17:00:00")),
        ("a", Some("2026-11-10T17:00:00")),
        ("none", None),
        ("d", Some("2026-12-31T17:00:00")),
    ] {
        let mut task = Task::new(title);
        task.due = due.map(Into::into);
        task.time_zone = due.map(|_| "Etc/UTC".to_string());
        r.create(task).await.unwrap();
    }
    let titles = |v: Vec<fs_tasks_core::TaskEntry>| -> Vec<String> {
        v.into_iter().map(|e| e.task.title.unwrap()).collect()
    };
    assert_eq!(titles(r.list().await.unwrap()), ["a", "c", "d", "none"]);
    let q = TaskQuery::new().due_between(t("2026-11-11T00:00:00Z"), t("2026-12-31T17:00:00Z"));
    assert_eq!(titles(r.query(&q).await.unwrap()), ["c"], "[from, to)");
    let q = TaskQuery {
        due_from: Some(t("2026-11-11T00:00:00Z")),
        ..TaskQuery::new()
    };
    assert_eq!(titles(r.query(&q).await.unwrap()), ["c", "d"], "open end");
}

#[tokio::test]
async fn query_by_status_and_label() {
    let r = repo().await;
    let mut done = Task::new("done");
    done.progress = Some("completed".into());
    r.create(done).await.unwrap();
    let open = r.create(Task::new("open")).await.unwrap();
    r.label(open.id, "urgent").await.unwrap();
    let names = |v: Vec<fs_tasks_core::TaskEntry>| -> Vec<String> {
        v.into_iter().map(|e| e.task.title.unwrap()).collect()
    };
    let q = TaskQuery::new().with_progress("completed");
    assert_eq!(names(r.query(&q).await.unwrap()), ["done"]);
    let q = TaskQuery::new()
        .with_progress("needs-action")
        .with_label("urgent");
    assert_eq!(names(r.query(&q).await.unwrap()), ["open"]);
    let q = TaskQuery::new().with_label("other");
    assert_eq!(
        r.query(&q).await.unwrap(),
        [] as [fs_tasks_core::TaskEntry; 0]
    );
}
