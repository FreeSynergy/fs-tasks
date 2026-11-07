//! The problems of the server come back as the errors of the core.
//!
//! Pattern: Integration tests against the trait; no stand-ins.

mod common;

use common::{Served, task};
use fs_store::ObjectId;
use fs_tasks_core::{Error, TaskItems};

#[tokio::test(flavor = "multi_thread")]
async fn not_found_names_the_id_of_the_call() {
    let served = Served::start().await;
    let tasks = served.client();
    let id = ObjectId::new();
    let got = tasks.delete(id).await;
    assert!(matches!(got, Err(Error::NotFound(i)) if i == id), "{got:?}");
    let got = tasks.edit(id, task("x", "X")).await;
    assert!(matches!(got, Err(Error::NotFound(i)) if i == id), "{got:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_taken_uid_is_a_conflict_with_the_servers_words() {
    let served = Served::start().await;
    let tasks = served.client();
    tasks.create(task("same", "One")).await.unwrap();
    let wire = tasks.create(task("same", "Two")).await.unwrap_err();
    let core = served.repo.create(task("same", "Two")).await.unwrap_err();
    assert!(matches!(wire, Error::Conflict(_)), "{wire:?}");
    assert_eq!(wire.to_string(), core.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_invalid_task_comes_back_with_its_issues() {
    let served = Served::start().await;
    let tasks = served.client();
    let mut bad = task("bad", "Bad");
    bad.progress = Some("sleeping".into());
    let wire = tasks.create(bad.clone()).await.unwrap_err();
    let core = served.repo.create(bad).await.unwrap_err();
    let (Error::Invalid(wire_issues), Error::Invalid(core_issues)) = (&wire, &core) else {
        panic!("{wire:?} / {core:?}");
    };
    assert_eq!(wire_issues, core_issues);
    assert_eq!(wire.to_string(), core.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_changed_uid_is_a_conflict_too() {
    let served = Served::start().await;
    let tasks = served.client();
    let made = tasks.create(task("keep", "T")).await.unwrap();
    let mut other = made.task.clone();
    other.uid = "changed".into();
    let got = tasks.edit(made.id, other).await;
    assert!(matches!(got, Err(Error::Conflict(_))), "{got:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn what_the_server_cannot_do_is_unsupported_with_its_capability() {
    let served = Served::start_read_only().await;
    let tasks = served.client();
    let caps = tasks.capabilities();
    assert!(!caps.is_supported("task.create"));
    let got = tasks.create(task("u", "T")).await;
    assert!(
        matches!(&got, Err(Error::Unsupported(u)) if u.capability() == "task.create"),
        "{got:?}"
    );
    let got = tasks.delete(ObjectId::new()).await;
    assert!(
        matches!(&got, Err(Error::Unsupported(u)) if u.capability() == "task.delete"),
        "{got:?}"
    );
    let got = tasks
        .import_ics("BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n")
        .await;
    assert!(
        matches!(&got, Err(Error::Unsupported(u)) if u.capability() == "task.import"),
        "{got:?}"
    );
    assert!(
        tasks.list().await.unwrap().is_empty(),
        "the reads still work"
    );
}
