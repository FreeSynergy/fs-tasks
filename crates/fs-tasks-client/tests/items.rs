//! `TaskItems` over the socket: the methods the app calls, compared by value against the provider
//! behind the real server.
//!
//! Integration tests against the trait; nothing stands in for the server or the store.

mod common;

use common::{Served, due, task};
use fs_store::ObjectId;
use fs_tasks_core::{TaskItems, TaskQuery};

#[tokio::test(flavor = "multi_thread")]
async fn a_task_goes_through_create_get_list_edit_delete() {
    let served = Served::start().await;
    let tasks = served.client();

    let made = tasks.create(due("uid-1", "First", 1)).await.unwrap();
    assert_eq!(made.task.title.as_deref(), Some("First"));
    assert_eq!(made.task.uid, "uid-1");

    let got = tasks.get(made.id).await.unwrap().unwrap();
    assert_eq!(got.task, made.task);
    assert_eq!(got.id, made.id);

    let listed = tasks.list().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].task, made.task);

    let mut changed = got.task.clone();
    changed.title = Some("Renamed".into());
    changed.due = Some("2026-12-02T10:00:00".into());
    let edited = tasks.edit(made.id, changed).await.unwrap();
    assert_eq!(edited.task.title.as_deref(), Some("Renamed"));
    let at_server = served.repo.get(made.id).await.unwrap().unwrap();
    assert_eq!(
        at_server.task, edited.task,
        "the server holds what came back"
    );
    assert_eq!(
        at_server.task.sequence,
        Some(1),
        "a new due date is a new sequence"
    );

    tasks.delete(made.id).await.unwrap();
    assert!(tasks.get(made.id).await.unwrap().is_none());
    assert!(tasks.list().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_id_is_none_not_an_error() {
    let served = Served::start().await;
    let tasks = served.client();
    assert!(tasks.get(ObjectId::new()).await.unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_envelope_arrives_as_the_store_has_it() {
    let served = Served::start().await;
    let tasks = served.client();
    let made = tasks.create(due("uid-env", "Envelope", 5)).await.unwrap();
    let at_server = served.repo.get(made.id).await.unwrap().unwrap().envelope;
    let e = made.envelope;
    assert_eq!(e.id, made.id);
    assert_eq!(e.object_type, "Task");
    assert_eq!(e.name, "Envelope");
    assert_eq!(e.context.as_deref(), Some("tasks"));
    assert_eq!(e.iri, at_server.iri);
    assert_eq!(e.attributed_to, at_server.attributed_to);
    assert_eq!(
        e.start_time.map(|t| t.timestamp_millis()),
        at_server.start_time.map(|t| t.timestamp_millis())
    );
    assert_eq!(
        e.end_time.map(|t| t.timestamp_millis()),
        at_server.end_time.map(|t| t.timestamp_millis())
    );
    assert!(e.end_time.is_some(), "the due date is the end of the span");
    assert_eq!(
        e.published.timestamp_millis(),
        at_server.published.timestamp_millis()
    );
    assert_eq!(
        e.updated.timestamp_millis(),
        at_server.updated.timestamp_millis()
    );
    assert!(e.sensitive.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_order_of_the_list_is_the_servers() {
    let served = Served::start().await;
    let tasks = served.client();
    tasks.create(due("c", "Late", 20)).await.unwrap();
    tasks.create(task("n", "Undated")).await.unwrap();
    tasks.create(due("a", "Early", 3)).await.unwrap();
    let titles: Vec<_> = tasks
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.task.title.unwrap())
        .collect();
    assert_eq!(titles, ["Early", "Late", "Undated"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_capabilities_are_what_the_server_reported() {
    let served = Served::start().await;
    let caps = served.client().capabilities();
    for id in [
        "task.create",
        "task.edit",
        "task.delete",
        "task.import",
        "task.export",
    ] {
        assert!(caps.is_supported(id), "{id}");
    }
    let report = served.repo.capabilities();
    assert_eq!(
        caps.is_supported("task.assign"),
        report.is_supported("task.assign")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn what_the_app_does_not_use_says_unsupported() {
    let served = Served::start().await;
    let tasks = served.client();
    let made = tasks.create(task("u", "T")).await.unwrap();
    for got in [
        tasks.get_by_uid("u").await.map(|_| ()),
        tasks.query(&TaskQuery::new()).await.map(|_| ()),
        tasks.assign(made.id, "Ada", "a@x.org").await.map(|_| ()),
        tasks.label(made.id, "x").await.map(|_| ()),
        tasks
            .relate(made.id, "other", fs_tasks_core::Relation::Child)
            .await
            .map(|_| ()),
    ] {
        assert!(
            matches!(got, Err(fs_tasks_core::Error::Unsupported(_))),
            "{got:?}"
        );
    }
    // nothing was sent: the task is as it was
    assert_eq!(
        served.repo.get(made.id).await.unwrap().unwrap().task,
        made.task
    );
}
