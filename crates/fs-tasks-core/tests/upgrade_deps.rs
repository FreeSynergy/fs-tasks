//! A store of 0.0.4 has no dependency fields (`rel_index`, `precedes`, `depends_on`) in its index. The data is made the
//! way 0.0.4 left it: tasks written by the current code, then those fields taken out of their index. Before the upgrade
//! the reverse lookups are blind (that is what the upgrade is for); after it they are right, and a second run does nothing.

mod common;

use common::{ME, setup};
use fs_tasks_core::plan::Readiness;
use fs_tasks_core::{Relation, Task, TaskItems};

async fn make(r: &fs_tasks_core::TaskRepo, uid: &str) -> fs_tasks_core::TaskEntry {
    let mut t = Task::new(uid);
    t.uid = uid.into();
    r.create(t).await.unwrap()
}

#[tokio::test]
async fn a_0_0_4_store_gets_its_dependency_fields_from_the_upgrade() {
    let (r, store) = setup().await;
    let paint = make(&r, "paint").await;
    let carpet = make(&r, "carpet").await;
    let tiles = make(&r, "tiles").await;
    r.relate(paint.id, "carpet", Relation::FinishToStart)
        .await
        .unwrap();
    r.relate(tiles.id, "paint", Relation::DependsOn)
        .await
        .unwrap();

    let content_before = r.get(paint.id).await.unwrap().unwrap().task;
    // The 0.0.4 state: the three fields are not there.
    for id in [paint.id, carpet.id, tiles.id] {
        let mut object = store.get(id).await.unwrap().unwrap();
        for key in ["rel_index", "precedes", "depends_on"] {
            object.index.remove(key);
        }
        store.put(object, ME).await.unwrap();
    }
    // Blind before: carpet looks ready (nothing is found that precedes it), paint does not know who depends on it.
    assert_eq!(r.readiness(carpet.id).await.unwrap(), Readiness::Ready);
    assert!(
        r.dependents(paint.id)
            .await
            .unwrap()
            .iter()
            .all(|e| e.task.uid != "tiles")
    );

    let report = r.upgrade_index().await.unwrap();
    assert_eq!(
        (report.rewritten, report.failed.len()),
        (3, 0),
        "{report:?}"
    );
    assert_eq!(
        r.readiness(carpet.id).await.unwrap(),
        Readiness::Blocked(vec!["paint".into()])
    );
    let dependents: Vec<String> = r
        .dependents(paint.id)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.task.uid)
        .collect();
    assert_eq!(dependents, ["carpet", "tiles"]);
    assert_eq!(
        r.readiness(tiles.id).await.unwrap(),
        Readiness::Blocked(vec!["paint".into()])
    );
    assert_eq!(r.upgrade_index().await.unwrap().rewritten, 0, "idempotent");
    // the content of the tasks is untouched
    assert_eq!(
        r.get(paint.id).await.unwrap().unwrap().task,
        content_before,
        "only the index was completed"
    );
}
