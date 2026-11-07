//! `edit_checked` through the client: compare-and-set against the real server and the real store.
//!
//! Integration tests against the trait; no stand-ins.

mod common;

use common::{Served, task};
use fs_store::ObjectId;
use fs_tasks_client::RevisedTasks;
use fs_tasks_core::{Error, Revision, Task, TaskItems};

/// The revision the server gives (the client cannot derive it: the wire keeps time in milliseconds).
async fn read(tasks: &dyn RevisedTasks, id: ObjectId) -> (Revision, Task) {
    let read = tasks.get_revised(id).await.unwrap().unwrap();
    (read.revision, read.entry.task)
}

#[tokio::test(flavor = "multi_thread")]
async fn the_revision_read_writes_and_a_stale_one_writes_nothing() {
    let served = Served::start().await;
    let tasks = served.client();
    let id = tasks.create(task("a", "Ada")).await.unwrap().id;
    let (revision, mut mine) = read(&tasks, id).await;

    mine.title = Some("First".into());
    let done = tasks.edit_checked(id, revision, mine).await.unwrap();
    assert_eq!(done.id, id);
    let (now, _) = read(&tasks, id).await;
    assert_ne!(now, revision, "a write changes the revision");

    let (_, mut other) = read(&tasks, id).await;
    other.title = Some("Second".into());
    let refused = tasks.edit_checked(id, revision, other).await;
    assert!(
        matches!(refused, Err(Error::Changed(got)) if got == id),
        "{refused:?}"
    );
    let (_, kept) = read(&tasks, id).await;
    assert_eq!(kept.title.as_deref(), Some("First"), "nothing was written");
}

#[tokio::test(flavor = "multi_thread")]
async fn two_writers_with_the_same_revision_have_one_winner() {
    let served = Served::start().await;
    let (one, two) = (served.client(), served.client());
    let id = one.create(task("a", "Ada")).await.unwrap().id;
    let (revision, base) = read(&one, id).await;
    let (mut a, mut b) = (base.clone(), base);
    a.title = Some("A".into());
    b.title = Some("B".into());
    let (ra, rb) = tokio::join!(
        one.edit_checked(id, revision, a),
        two.edit_checked(id, revision, b)
    );
    assert_eq!(u8::from(ra.is_ok()) + u8::from(rb.is_ok()), 1, "{ra:?} {rb:?}");
    let loser = if ra.is_ok() { rb } else { ra };
    assert!(matches!(loser, Err(Error::Changed(_))), "{loser:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_list_carries_the_same_revisions_as_get() {
    let served = Served::start().await;
    let tasks = served.client();
    let id = tasks.create(task("a", "Ada")).await.unwrap().id;
    let listed = tasks.list_revised().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].entry.id, id);
    let (revision, _) = read(&tasks, id).await;
    assert_eq!(listed[0].revision, revision);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_task_is_not_found_and_the_client_offers_it() {
    let served = Served::start().await;
    let tasks = served.client();
    let ghost = ObjectId::new();
    let got = tasks
        .edit_checked(ghost, Revision::from(1), task("x", "X"))
        .await;
    assert!(matches!(got, Err(Error::NotFound(_))), "{got:?}");
    assert!(tasks.capabilities().is_supported("task.edit"));
    assert!(tasks.get_revised(ghost).await.unwrap().is_none());
}
