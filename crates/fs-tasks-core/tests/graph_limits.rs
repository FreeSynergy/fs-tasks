//! The limits of the dependency questions, with the limits lowered (`TaskRepo::with_graph_limits`): a question that
//! would load more than the limit is refused with a conflict that says so, never answered half.

mod common;

use std::sync::Arc;

use common::ME;
use fs_objects::{Store, TursoStore};
use fs_tasks_core::{Error, Relation, Task, TaskItems, TaskQuery, TaskRepo};

/// 40 tasks in a chain, a store whose graph limit is 30 and whose scan limit is 50.
async fn chain(n: usize) -> (TaskRepo, Vec<fs_objects::ObjectId>) {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let r = TaskRepo::new(store, ME).with_graph_limits(30, 50);
    let mut ids = Vec::new();
    for i in 0..n {
        let mut t = Task::new(format!("t{i}"));
        t.uid = format!("t{i}");
        if i + 1 < n {
            t.relate(&format!("t{}", i + 1), Relation::FinishToStart);
        }
        ids.push(r.create(t).await.unwrap().id);
    }
    (r, ids)
}

fn says(e: Result<impl std::fmt::Debug, Error>, text: &str) {
    match e {
        Err(Error::Conflict(m)) => assert!(m.contains(text), "{m}"),
        other => panic!("expected a conflict about {text:?}, got {other:?}"),
    }
}

#[tokio::test]
async fn a_graph_larger_than_the_limit_is_refused_by_every_way_of_asking() {
    let (r, ids) = chain(40).await;
    says(r.readiness_many(&ids).await, "more than 30 tasks");
    says(r.readiness(ids[0]).await, "more than 30 tasks");
    says(
        r.page(&TaskQuery::new().only_ready(true), None, 5).await,
        "more than 30 tasks",
    );
    // the other query path of the filter (a label forces the store query and the graph scan)
    says(
        r.query(&TaskQuery::new().only_ready(true).with_label("x"))
            .await,
        "more than 30 tasks",
    );
    says(
        r.relate(ids[39], "t0", Relation::FinishToStart).await,
        "more than 30 tasks",
    );
    // what does not need the graph is not touched
    assert_eq!(r.list().await.unwrap().len(), 40);
    assert!(
        r.relate(ids[0], "elsewhere", Relation::Parent)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn a_graph_within_the_limit_is_answered() {
    let (r, ids) = chain(25).await;
    assert_eq!(r.readiness_many(&ids).await.unwrap().len(), 25);
    assert_eq!(
        r.page(&TaskQuery::new().only_ready(true), None, 5)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
}

#[tokio::test]
async fn a_store_larger_than_the_scan_limit_is_refused_for_a_scan_but_not_for_a_list() {
    let (r, _ids) = chain(10).await;
    for i in 10..60 {
        let mut t = Task::new(format!("x{i}"));
        t.uid = format!("x{i}");
        r.create(t).await.unwrap();
    }
    says(
        r.page(&TaskQuery::new().only_ready(true), None, 5).await,
        "more than 50 tasks",
    );
    assert_eq!(r.list().await.unwrap().len(), 60);
    assert_eq!(
        r.page(&TaskQuery::new(), None, 5)
            .await
            .unwrap()
            .items
            .len(),
        5
    );
}
