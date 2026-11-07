//! `TaskItems::edit_checked`, tested against the trait: the task is replaced only if it still has the
//! revision the caller read; of two writers on one revision exactly one wins; a wrong revision writes
//! nothing.

mod common;

use common::yielding::{join_all, yielding_repo};
use common::{repo, setup};
use fs_tasks_core::{Error, Revision, Task, TaskItems};

fn titled(of: &Task, title: &str) -> Task {
    let mut task = of.clone();
    task.title = Some(title.into());
    task
}

#[tokio::test]
async fn the_revision_read_writes_and_the_next_one_differs() {
    let r = repo().await;
    let first = r.create(Task::new("Plan")).await.unwrap();
    let seen = Revision::of(&first);
    assert_eq!(Revision::of(&r.get(first.id).await.unwrap().unwrap()), seen);

    let second = r
        .edit_checked(first.id, seen, titled(&first.task, "Plan more"))
        .await
        .unwrap();
    assert_eq!(second.envelope.name, "Plan more");
    assert_ne!(
        Revision::of(&second),
        seen,
        "every write changes the revision"
    );
    let read = r.get(first.id).await.unwrap().unwrap();
    assert_eq!(Revision::of(&read), Revision::of(&second));
}

#[tokio::test]
async fn a_wrong_revision_writes_nothing_and_does_not_name_the_right_one() {
    let (r, store) = setup().await;
    let first = r.create(Task::new("Plan")).await.unwrap();
    let stale = Revision::of(&first);
    let current = r
        .edit(first.id, titled(&first.task, "Plan B"))
        .await
        .unwrap();
    let before = store.get(first.id).await.unwrap().unwrap();

    let err = r
        .edit_checked(first.id, stale, titled(&first.task, "Plan C"))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Changed(id) if id == first.id), "{err}");
    let text = err.to_string();
    assert!(text.starts_with("conflict:"), "{text}");
    assert!(
        !text.contains(&Revision::of(&current).to_string()),
        "{text}"
    );

    let after = store.get(first.id).await.unwrap().unwrap();
    assert_eq!(after.content, before.content, "nothing was written");
    assert_eq!(after.envelope.updated, before.envelope.updated);
    assert_eq!(
        Revision::of(&r.get(first.id).await.unwrap().unwrap()),
        Revision::of(&current)
    );
}

#[tokio::test]
async fn an_unknown_task_is_not_found() {
    let r = repo().await;
    let err = r
        .edit_checked(fs_store::ObjectId::new(), Revision::from(1), Task::new("x"))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::NotFound(_)), "{err}");
}

#[tokio::test]
async fn of_two_writers_on_one_revision_exactly_one_wins() {
    for round in 0..10 {
        let r = yielding_repo().await;
        let first = r.create(Task::new("Plan")).await.unwrap();
        let seen = Revision::of(&first);
        let writers = ["Writer A", "Writer B"].into_iter().map(|title| {
            let (r, task, id) = (r.clone(), titled(&first.task, title), first.id);
            async move { (title, r.edit_checked(id, seen, task).await) }
        });
        let results = join_all(writers).await;
        let won: Vec<_> = results.iter().filter(|(_, r)| r.is_ok()).collect();
        let lost = results
            .iter()
            .filter(|(_, r)| matches!(r, Err(Error::Changed(_))))
            .count();
        assert_eq!((won.len(), lost), (1, 1), "round {round}");
        let now = r.get(first.id).await.unwrap().unwrap();
        assert_eq!(now.envelope.name, won[0].0, "the loser wrote nothing");
    }
}
