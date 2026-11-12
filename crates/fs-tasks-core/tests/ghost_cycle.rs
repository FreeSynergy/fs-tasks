//! A cycle that runs through a task the store does not hold: a FS g, b depends on g, b FS c, c FS a, and `g` is not in the
//! store. The tasks that depend on a missing task are followed, so a, b and c are in a cycle: a conflict, however it is
//! asked (the group names the missing task too; one task, many tasks, the `ready` list by scan and by the store query, and its pages).

mod common;

use common::repo;
use fs_tasks_core::plan::Readiness;
use fs_tasks_core::{Relation, Task, TaskItems, TaskQuery};

#[tokio::test]
async fn a_cycle_through_a_task_the_store_does_not_hold_is_a_conflict_every_way() {
    let r = repo().await;
    let mut ids = Vec::new();
    for (uid, rels) in [
        ("a", vec![("g", Relation::FinishToStart)]),
        (
            "b",
            vec![("g", Relation::DependsOn), ("c", Relation::FinishToStart)],
        ),
        ("c", vec![("a", Relation::FinishToStart)]),
    ] {
        let mut t = Task::new(uid);
        t.uid = uid.into();
        t.label("x");
        for (other, kind) in rels {
            t.relate(other, kind);
        }
        ids.push(r.create(t).await.unwrap().id);
    }
    let group: Vec<String> = ["a", "b", "c", "g"].map(String::from).to_vec();
    for id in &ids {
        assert_eq!(
            r.readiness(*id).await.unwrap(),
            Readiness::Conflict(group.clone()),
            "one task"
        );
    }
    for (_, readiness) in r.readiness_many(&ids).await.unwrap() {
        assert_eq!(readiness, Readiness::Conflict(group.clone()), "many tasks");
    }
    // the ready list and its pages: nothing is ready, everything is not ready (scan path, then the store-query path)
    for query in [TaskQuery::new(), TaskQuery::new().with_label("x")] {
        let ready = query.clone().only_ready(true);
        let waiting = query.only_ready(false);
        assert_eq!(r.query(&ready).await.unwrap(), [], "ready list");
        assert_eq!(r.query(&waiting).await.unwrap().len(), 3, "not-ready list");
        assert_eq!(
            r.page(&ready, None, 2).await.unwrap().items,
            [],
            "ready page"
        );
        let page = r.page(&waiting, None, 2).await.unwrap();
        assert_eq!((page.items.len(), page.more), (2, true));
        let next = r
            .page(&waiting, Some(&page.items[1].place), 2)
            .await
            .unwrap();
        assert_eq!((next.items.len(), next.more), (1, false));
    }
}
