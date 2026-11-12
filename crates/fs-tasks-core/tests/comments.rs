//! Comments on tasks: own objects, one level of thread, tombstones, the iCalendar COMMENT lines.

mod common;

use std::sync::{Arc, Mutex};

use common::{ME, repo, setup};
use fs_tasks_core::{
    Comments, Error, Task, TaskChange, TaskEntry, TaskFeed, TaskItems, TaskListener, TaskQuery,
    TaskRepo,
};

async fn task(r: &TaskRepo, title: &str) -> (TaskEntry, String) {
    let e = r.create(Task::new(title)).await.unwrap();
    let iri = e.envelope.iri.clone().unwrap();
    (e, iri)
}

#[tokio::test]
async fn a_comment_is_its_own_object_with_the_author_the_provider_sets() {
    let (r, store) = setup().await;
    let (t, iri) = task(&r, "Plan").await;
    let c = r.comment_add(&iri, "  First!\nmore  ", None).await.unwrap();
    assert_eq!(c.body, "First!\nmore");
    assert_eq!((c.author(), c.context()), (ME, iri.as_str()));
    assert_eq!(c.envelope.object_type, "Note");
    assert_eq!(c.envelope.name, "First!");
    assert!(c.in_reply_to().is_none() && !c.tombstone);
    // not part of the task: the task is unchanged and a query of tasks does not see the comment
    assert_eq!(r.get(t.id).await.unwrap().unwrap().task, t.task);
    assert_eq!(r.list().await.unwrap().len(), 1);
    assert!(
        r.query(&TaskQuery::new().matching("First"))
            .await
            .unwrap()
            .is_empty()
    );
    let _ = store;
    // refused: empty text, a context that is no task
    assert!(matches!(
        r.comment_add(&iri, "  ", None).await,
        Err(Error::Rejected(_))
    ));
    assert!(
        r.comment_add("https://x/tasks/nope", "x", None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn threads_are_one_level_deep_and_a_reply_follows_its_comment() {
    let r = repo().await;
    let (_t, iri) = task(&r, "Plan").await;
    let (_t2, other) = task(&r, "Other").await;
    let a = r.comment_add(&iri, "a", None).await.unwrap();
    let b = r.comment_add(&iri, "b", None).await.unwrap();
    let reply = r
        .comment_add(&iri, "re a", a.envelope.iri.as_deref())
        .await
        .unwrap();
    assert_eq!(reply.in_reply_to(), a.envelope.iri.as_deref());
    let order: Vec<_> = r
        .comments(&iri)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.body)
        .collect();
    assert_eq!(order, ["a", "re a", "b"]);
    // a reply to a reply, to a comment of another task, or to nothing: invalid reply
    for parent in [
        reply.envelope.iri.clone().unwrap(),
        b.envelope.iri.clone().unwrap(),
    ] {
        let into_other = r.comment_add(&other, "x", Some(&parent)).await;
        assert!(
            matches!(into_other, Err(Error::InvalidReply(_))),
            "{parent}"
        );
    }
    assert!(matches!(
        r.comment_add(&iri, "x", reply.envelope.iri.as_deref())
            .await,
        Err(Error::InvalidReply(_))
    ));
    assert!(matches!(
        r.comment_add(&iri, "x", Some("https://x/comments/nothing"))
            .await,
        Err(Error::InvalidReply(_))
    ));
}

#[tokio::test]
async fn edit_is_compare_and_set_and_delete_leaves_a_tombstone_that_keeps_the_thread() {
    let r = repo().await;
    let (_t, iri) = task(&r, "Plan").await;
    let a = r.comment_add(&iri, "a", None).await.unwrap();
    r.comment_add(&iri, "re a", a.envelope.iri.as_deref())
        .await
        .unwrap();
    let edited = r.comment_edit(a.id, a.revision(), "a2").await.unwrap();
    assert_eq!(edited.body, "a2");
    assert!(matches!(
        r.comment_edit(a.id, a.revision(), "stale").await,
        Err(Error::Changed(_))
    ));
    let gone = r.comment_delete(a.id).await.unwrap();
    assert!(gone.tombstone && gone.body.is_empty() && gone.deleted.is_some());
    assert_eq!(gone.envelope.object_type, "Tombstone");
    let thread = r.comments(&iri).await.unwrap();
    assert_eq!(thread.len(), 2, "the reply is still under its tombstone");
    assert!(thread[0].tombstone && thread[1].body == "re a");
    assert!(r.comment_edit(a.id, gone.revision(), "back").await.is_err());
    assert_eq!(r.comment_delete(a.id).await.unwrap().id, a.id, "idempotent");
}

#[tokio::test]
async fn only_the_author_or_the_task_owner_may_change_a_comment() {
    let (r, store) = setup().await;
    let (_t, iri) = task(&r, "Plan").await;
    let mine = r.comment_add(&iri, "mine", None).await.unwrap();
    // the same store seen as another actor
    let other = TaskRepo::new(store, "https://local.invalid/users/ada");
    assert!(matches!(
        other.comment_edit(mine.id, mine.revision(), "x").await,
        Err(Error::Forbidden(_))
    ));
    // ada is no owner of the task either
    assert!(matches!(
        other.comment_delete(mine.id).await,
        Err(Error::Forbidden(_))
    ));
    // the owner (me) may delete what ada wrote
    let hers = other.comment_add(&iri, "hers", None).await.unwrap();
    assert_eq!(hers.author(), "https://local.invalid/users/ada");
    assert!(r.comment_delete(hers.id).await.unwrap().tombstone);
}

#[tokio::test]
async fn deleting_the_task_takes_its_comments_and_a_comment_is_announced() {
    let (r, store) = setup().await;
    let (t, iri) = task(&r, "Plan").await;
    struct Log(Mutex<Vec<TaskChange>>);
    impl TaskListener for Log {
        fn changed(&self, c: &TaskChange) {
            self.0.lock().unwrap().push(*c);
        }
    }
    let log = Arc::new(Log(Mutex::default()));
    let _sub = r.subscribe(log.clone());
    let c = r.comment_add(&iri, "hello", None).await.unwrap();
    assert!(log.0.lock().unwrap().contains(&TaskChange::Commented {
        task: t.id,
        comment: c.id
    }));
    r.delete(t.id).await.unwrap();
    use fs_objects::Query;
    let left = store
        .query(&Query::new().in_context(iri.as_str()))
        .await
        .unwrap();
    assert!(left.is_empty(), "no comment is left behind: {}", left.len());
}

#[tokio::test]
async fn vtodo_comment_lines_become_comments_and_go_out_again() {
    let r = repo().await;
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nUID:todo-c\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:Minutes\r\nCOMMENT:first\\, with comma\\nand a line\r\nCOMMENT;LANGUAGE=de:zweite\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
    let report = r.import_ics(ics).await.unwrap();
    assert_eq!(report.created.len(), 1, "{report:?}");
    assert!(
        report
            .notes
            .iter()
            .any(|n| n.contains("2 COMMENT") && n.contains("importing actor")),
        "{:?}",
        report.notes
    );
    let t = r.get(report.created[0]).await.unwrap().unwrap();
    let texts: Vec<_> = r
        .comments(t.envelope.iri.as_deref().unwrap())
        .await
        .unwrap()
        .into_iter()
        .map(|c| {
            let a = c.author().to_string();
            (c.body, a)
        })
        .collect();
    assert_eq!(
        texts,
        [
            ("first, with comma\nand a line".to_string(), ME.to_string()),
            ("zweite".to_string(), ME.to_string())
        ]
    );
    assert!(
        !t.task
            .to_value()
            .to_string()
            .to_lowercase()
            .contains("zweite"),
        "not kept in the task"
    );
    // out again: oldest first, a deleted one left out
    let iri = t.envelope.iri.clone().unwrap();
    let third = r.comment_add(&iri, "dritte", None).await.unwrap();
    let gone = r.comment_add(&iri, "weg", None).await.unwrap();
    r.comment_delete(gone.id).await.unwrap();
    let out = r.export_ics(&[t.id]).await.unwrap();
    let comments: Vec<&str> = out.lines().filter(|l| l.starts_with("COMMENT")).collect();
    assert_eq!(comments.len(), 3, "{out}");
    assert!(comments[0].contains("first\\, with comma"), "{out}");
    assert!(
        comments[2].ends_with("dritte") && !out.contains("weg"),
        "{out}"
    );
    let _ = third;
}
