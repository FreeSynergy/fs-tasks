//! Dependencies in the store: readiness, the neighbours, cycles refused, the `ready` filter, the unblocked event.

mod common;

use std::sync::{Arc, Mutex};

use common::{ME, repo};
use fs_tasks_core::plan::Readiness;
use fs_tasks_core::{
    Error, Relation, Task, TaskChange, TaskFeed, TaskItems, TaskListener, TaskQuery, TaskRepo,
};

async fn make(r: &TaskRepo, uid: &str, progress: &str) -> fs_tasks_core::TaskEntry {
    let mut t = Task::new(uid);
    t.uid = uid.into();
    t.progress = Some(progress.into());
    r.create(t).await.unwrap()
}

/// Paint holds finishtostart towards carpet (RFC 9253: the holder is the predecessor).
async fn paint_then_carpet() -> (TaskRepo, fs_tasks_core::TaskEntry, fs_tasks_core::TaskEntry) {
    let r = repo().await;
    let paint = make(&r, "paint", "needs-action").await;
    let carpet = make(&r, "carpet", "needs-action").await;
    r.relate(paint.id, "carpet", Relation::FinishToStart)
        .await
        .unwrap();
    (r, paint, carpet)
}

#[tokio::test]
async fn readiness_and_the_neighbours_in_the_store() {
    let (r, paint, carpet) = paint_then_carpet().await;
    assert_eq!(r.readiness(paint.id).await.unwrap(), Readiness::Ready);
    assert_eq!(
        r.readiness(carpet.id).await.unwrap(),
        Readiness::Blocked(vec!["paint".into()])
    );
    let by: Vec<_> = r.blocked_by(carpet.id).await.unwrap();
    assert_eq!(by.len(), 1);
    assert_eq!(by[0].task.uid, "paint");
    assert_eq!(r.dependents(paint.id).await.unwrap()[0].task.uid, "carpet");
    assert_eq!(r.blocked_by(paint.id).await.unwrap(), []);
    assert!(r.readiness(fs_objects::ObjectId::new()).await.is_err());
}

#[tokio::test]
async fn depends_on_is_found_from_both_sides() {
    let r = repo().await;
    let a = make(&r, "a", "needs-action").await;
    let b = make(&r, "b", "needs-action").await;
    // `b` depends on `a`: held by the dependent task.
    r.relate(b.id, "a", Relation::DependsOn).await.unwrap();
    assert_eq!(r.dependents(a.id).await.unwrap()[0].task.uid, "b");
    assert_eq!(
        r.readiness(b.id).await.unwrap(),
        Readiness::Blocked(vec!["a".into()])
    );
    assert_eq!(r.readiness(a.id).await.unwrap(), Readiness::Ready);
}

#[tokio::test]
async fn a_dependency_that_is_not_in_the_store_is_unknown() {
    let r = repo().await;
    let b = make(&r, "b", "needs-action").await;
    r.relate(b.id, "elsewhere", Relation::DependsOn)
        .await
        .unwrap();
    assert_eq!(
        r.readiness(b.id).await.unwrap(),
        Readiness::Unknown(vec!["elsewhere".into()])
    );
}

#[tokio::test]
async fn relate_refuses_a_cycle_and_writes_nothing() {
    let r = repo().await;
    let a = make(&r, "a", "needs-action").await;
    let b = make(&r, "b", "needs-action").await;
    let c = make(&r, "c", "needs-action").await;
    r.relate(a.id, "b", Relation::FinishToStart).await.unwrap();
    r.relate(b.id, "c", Relation::FinishToStart).await.unwrap();
    // c -> a would close a -> b -> c -> a; also written as a dependency in the other direction (a depends on c)
    let before = r.get(c.id).await.unwrap().unwrap();
    let err = r.relate(c.id, "a", Relation::FinishToStart).await;
    match err {
        Err(Error::Cycle(path)) => assert_eq!(path, ["a", "b", "c"]),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        r.relate(a.id, "c", Relation::DependsOn).await,
        Err(Error::Cycle(_))
    ));
    assert!(matches!(
        r.relate(a.id, "a", Relation::StartToStart).await,
        Err(Error::Cycle(_))
    ));
    assert_eq!(
        r.get(c.id).await.unwrap().unwrap(),
        before,
        "nothing written"
    );
    // a diamond edge is fine, and so are the relations that are no dependency
    r.relate(a.id, "c", Relation::FinishToStart).await.unwrap();
    r.relate(c.id, "a", Relation::Parent).await.unwrap();
    assert!(Error::Cycle(vec![]).is_conflict());
}

#[tokio::test]
async fn a_cycle_written_by_an_edit_is_a_conflict_on_read() {
    let r = repo().await;
    let a = make(&r, "a", "needs-action").await;
    let b = make(&r, "b", "needs-action").await;
    r.relate(a.id, "b", Relation::FinishToStart).await.unwrap();
    // an edit (a human, or a merge) can still write what relate refuses
    let mut back = b.task.clone();
    back.relate("a", Relation::FinishToStart);
    r.edit(b.id, back).await.unwrap();
    let both: Vec<String> = vec!["a".into(), "b".into()];
    assert_eq!(
        r.readiness(a.id).await.unwrap(),
        Readiness::Conflict(both.clone())
    );
    assert_eq!(r.readiness(b.id).await.unwrap(), Readiness::Conflict(both));
}

#[tokio::test]
async fn relate_with_gap_stores_the_member_and_refuses_what_has_no_meaning() {
    let r = repo().await;
    let a = make(&r, "a", "needs-action").await;
    make(&r, "b", "needs-action").await;
    let e = r
        .relate_with_gap(a.id, "b", Relation::FinishToStart, "P1D")
        .await
        .unwrap();
    assert_eq!(e.task.related_to["b"]["freesynergy.net:gap"], "P1D");
    assert!(
        r.relate_with_gap(a.id, "b", Relation::Parent, "P1D")
            .await
            .is_err()
    );
    assert!(
        r.relate_with_gap(a.id, "b", Relation::FinishToStart, "tomorrow")
            .await
            .is_err()
    );
    // the export writes it as the RFC 9253 parameter
    let ics = r.export_ics(&[a.id]).await.unwrap();
    assert!(ics.contains(";GAP=P1D:b"), "{ics}");
}

#[tokio::test]
async fn the_ready_filter_and_its_pages() {
    let (r, paint, _carpet) = paint_then_carpet().await;
    make(&r, "lone", "needs-action").await;
    make(&r, "old", "completed").await;
    let uids = |v: Vec<fs_tasks_core::TaskEntry>| {
        let mut u: Vec<String> = v.into_iter().map(|e| e.task.uid).collect();
        u.sort();
        u
    };
    assert_eq!(
        uids(r.query(&TaskQuery::new().only_ready(true)).await.unwrap()),
        ["lone", "paint"]
    );
    assert_eq!(
        uids(r.query(&TaskQuery::new().only_ready(false)).await.unwrap()),
        ["carpet", "old"]
    );
    // pages of one task each walk the same set, in order, with `more` right
    let query = TaskQuery::new().only_ready(true);
    let first = r.page(&query, None, 1).await.unwrap();
    assert_eq!((first.items.len(), first.more), (1, true));
    let second = r
        .page(&query, Some(&first.items[0].place), 1)
        .await
        .unwrap();
    assert_eq!((second.items.len(), second.more), (1, false));
    let mut got = [
        first.items[0].entry.task.uid.clone(),
        second.items[0].entry.task.uid.clone(),
    ];
    got.sort();
    assert_eq!(got, ["lone", "paint"]);
    // completing paint makes carpet ready
    let mut done = paint.task.clone();
    done.progress = Some("completed".into());
    r.edit(paint.id, done).await.unwrap();
    assert_eq!(
        uids(r.query(&TaskQuery::new().only_ready(true)).await.unwrap()),
        ["carpet", "lone"]
    );
}

struct Log(Mutex<Vec<TaskChange>>);
impl TaskListener for Log {
    fn changed(&self, change: &TaskChange) {
        self.0.lock().unwrap().push(*change);
    }
}

#[tokio::test]
async fn completing_a_task_announces_who_became_ready() {
    let (r, paint, carpet) = paint_then_carpet().await;
    let third = make(&r, "third", "needs-action").await;
    // `third` waits for `carpet` and `paint` (two predecessors): it stays blocked after paint alone
    r.relate(paint.id, "third", Relation::FinishToStart)
        .await
        .unwrap();
    r.relate(carpet.id, "third", Relation::FinishToStart)
        .await
        .unwrap();
    let log = Arc::new(Log(Mutex::default()));
    let _sub = r.subscribe(log.clone());
    let mut done = r.get(paint.id).await.unwrap().unwrap().task;
    done.progress = Some("completed".into());
    r.edit(paint.id, done).await.unwrap();
    let seen = log.0.lock().unwrap().clone();
    assert_eq!(
        seen,
        [
            TaskChange::Updated(paint.id),
            TaskChange::Unblocked(carpet.id)
        ],
        "carpet became ready, third still waits for carpet: {third:?}"
    );
    let _ = ME;
}

#[tokio::test]
async fn an_old_task_without_the_dependency_fields_is_completed_by_the_upgrade() {
    let (r, _paint, carpet) = paint_then_carpet().await;
    // what upgrade_index repairs is exactly what lacks `rel_index`; a fresh repository has nothing to repair
    let report = r.upgrade_index().await.unwrap();
    assert_eq!((report.rewritten, report.failed.len()), (0, 0));
    assert_eq!(
        r.readiness(carpet.id).await.unwrap(),
        Readiness::Blocked(vec!["paint".into()])
    );
}
