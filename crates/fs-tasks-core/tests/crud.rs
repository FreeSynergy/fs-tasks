//! Create, edit, delete, sequence handling, undo and identity rules.

mod common;

use common::{repo, setup};
use fs_objects::ObjectId;
use fs_tasks_core::{Error, Task, TaskItems};

#[tokio::test]
async fn create_get_edit_delete() {
    let (r, store) = setup().await;
    let e = r.create(Task::new("Plan")).await.unwrap();
    assert_eq!(e.task.version.as_deref(), Some("2.0"));
    assert_eq!(e.id.to_string(), e.task.uid.trim_start_matches("urn:uuid:"));
    let stored = store.get(e.id).await.unwrap().unwrap();
    assert_eq!(stored.envelope.name, "Plan");
    assert_eq!(stored.envelope.context.as_deref(), Some("tasks"));
    assert_eq!(
        stored.envelope.iri.as_deref(),
        Some(format!("https://local.invalid/tasks/{}", e.id).as_str())
    );
    assert_eq!(r.get_by_uid(&e.task.uid).await.unwrap().unwrap().id, e.id);
    // a duplicate uid is refused
    let dup = r.create(e.task.clone()).await;
    assert!(matches!(dup, Err(ref e) if e.is_conflict()));
    // sequence follows due changes, not title changes
    let mut t = e.task.clone();
    t.title = Some("Plan more".into());
    assert_eq!(r.edit(e.id, t.clone()).await.unwrap().task.sequence, None);
    t.due = Some("2026-12-01T10:00:00".into());
    let after = r.edit(e.id, t).await.unwrap();
    assert_eq!(after.task.sequence, Some(1));
    assert_eq!(
        store.get(e.id).await.unwrap().unwrap().envelope.name,
        "Plan more"
    );
    // uid cannot change; invalid tasks are refused with all issues
    let mut other = after.task.clone();
    other.uid = "x".into();
    assert!(matches!(r.edit(e.id, other).await, Err(Error::Conflict(_))));
    let mut bad = after.task.clone();
    bad.progress = Some("sleeping".into());
    assert!(matches!(r.edit(e.id, bad).await, Err(Error::Invalid(_))));
    // delete is undoable through the store
    r.delete(e.id).await.unwrap();
    assert!(r.get(e.id).await.unwrap().is_none());
    assert!(matches!(r.delete(e.id).await, Err(Error::NotFound(_))));
    assert!(matches!(
        r.edit(ObjectId::new(), Task::new("x")).await,
        Err(Error::NotFound(_))
    ));
}

#[tokio::test]
async fn assign_relate_and_label() {
    let r = repo().await;
    let a = r.create(Task::new("A")).await.unwrap();
    let b = r.create(Task::new("B")).await.unwrap();
    let e = r.assign(a.id, "Ben", "ben@example.org").await.unwrap();
    assert_eq!(e.task.participants.len(), 1);
    let e = r
        .relate(a.id, &b.task.uid, fs_tasks_core::Relation::DependsOn)
        .await
        .unwrap();
    assert_eq!(
        e.task.relations(),
        [(b.task.uid.clone(), fs_tasks_core::Relation::DependsOn)]
    );
    let e = r.label(a.id, "backend").await.unwrap();
    assert_eq!(e.task.keywords.get("backend"), Some(&true));
    assert!(e.task.updated.is_some());
    assert!(r.assign(ObjectId::new(), "x", "x@y.z").await.is_err());
}

#[tokio::test]
async fn capabilities_are_listed() {
    let caps = repo().await.capabilities();
    for id in [
        "task.create",
        "task.edit",
        "task.assign",
        "relation.create",
        "label.assign",
    ] {
        assert!(caps.is_supported(id), "{id}");
    }
}

#[tokio::test]
async fn a_taken_uid_names_the_existing_task() {
    let r = repo().await;
    let first = r.create(Task::new("One")).await.unwrap();
    let got = r.create(first.task.clone()).await;
    assert!(
        matches!(got, Err(Error::Exists(id)) if id == first.id),
        "{got:?}"
    );
}
