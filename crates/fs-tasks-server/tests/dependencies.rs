//! The dependency methods of the contract over a real socket: readiness, blockedBy, dependents, the gap, and the
//! cycle problem with its path.

mod common;

use common::{create, items_of, set_id, with_server};
use fs_interfaces::rpc::Problem;
use fs_interfaces::tasks_capnp::{Readiness, Relation};

const CYCLE: &str = "https://freesynergy.net/problems/tasks/cycle";

#[tokio::test(flavor = "current_thread")]
async fn readiness_dependents_and_blocked_by_answer_what_the_core_knows() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let paint = create(&items, "paint", "Paint").await;
        let carpet = create(&items, "carpet", "Carpet").await;
        let mut relate = items.create_relation_with_gap_request();
        set_id(relate.get().init_id(), paint);
        relate.get().set_other_uid("carpet");
        relate.get().set_relation(Relation::FinishToStart);
        relate.get().set_gap("P1D");
        let answer = relate.send().promise.await.unwrap();
        let stored = answer.get().unwrap().get_task().unwrap();
        let json: serde_json::Value =
            serde_json::from_str(stored.get_jscalendar().unwrap().to_str().unwrap()).unwrap();
        assert_eq!(json["relatedTo"]["carpet"]["freesynergy.net:gap"], "P1D");

        let mut ask = items.readiness_request();
        set_id(ask.get().init_id(), carpet);
        let answer = ask.send().promise.await.unwrap();
        let report = answer.get().unwrap().get_report().unwrap();
        assert_eq!(report.get_readiness().unwrap(), Readiness::Blocked);
        let by: Vec<_> = report
            .get_by()
            .unwrap()
            .iter()
            .map(|t| t.unwrap().to_str().unwrap().to_string())
            .collect();
        assert_eq!(by, ["paint"]);

        let mut ask = items.readiness_request();
        set_id(ask.get().init_id(), paint);
        let answer = ask.send().promise.await.unwrap();
        assert_eq!(
            answer
                .get()
                .unwrap()
                .get_report()
                .unwrap()
                .get_readiness()
                .unwrap(),
            Readiness::Ready
        );

        let mut blocked = items.blocked_by_request();
        set_id(blocked.get().init_id(), carpet);
        let answer = blocked.send().promise.await.unwrap();
        assert_eq!(answer.get().unwrap().get_tasks().unwrap().len(), 1);

        let mut dependents = items.dependents_request();
        set_id(dependents.get().init_id(), paint);
        let answer = dependents.send().promise.await.unwrap();
        assert_eq!(answer.get().unwrap().get_tasks().unwrap().len(), 1);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_cycle_is_refused_as_the_cycle_problem_with_its_path() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let a = create(&items, "a", "A").await;
        let b = create(&items, "b", "B").await;
        let mut ab = items.create_relation_request();
        set_id(ab.get().init_id(), a);
        ab.get().set_other_uid("b");
        ab.get().set_relation(Relation::FinishToStart);
        ab.send().promise.await.unwrap();
        let mut ba = items.create_relation_request();
        set_id(ba.get().init_id(), b);
        ba.get().set_other_uid("a");
        ba.get().set_relation(Relation::FinishToStart);
        let error = ba.send().promise.await.err().unwrap();
        let p = Problem::from_error(&error).expect("a typed problem");
        assert_eq!((p.problem_type.as_str(), p.status), (CYCLE, 409));
        assert_eq!(p.ext_all("path").collect::<Vec<_>>(), ["a", "b"]);

        // a gap on a relation without temporal meaning is refused as a conflict, nothing is written
        let mut gap = items.create_relation_with_gap_request();
        set_id(gap.get().init_id(), a);
        gap.get().set_other_uid("b");
        gap.get().set_relation(Relation::Parent);
        gap.get().set_gap("P1D");
        assert!(gap.send().promise.await.is_err());
    })
    .await;
}
