//! Dependency graph, readiness and cycles (pure functions, RFC 9253 direction). The schedule is in `schedule.rs`.

use chrono::Duration;
use fs_tasks_core::plan::{
    Edge, EdgeKind, Readiness, cycles, edges_of, path_closing_cycle, readiness_of,
};
use fs_tasks_core::{Relation, Task};

fn task(uid: &str, progress: &str) -> Task {
    let mut t = Task::new(uid);
    t.uid = uid.into();
    t.progress = Some(progress.into());
    t
}

fn e(from: &str, to: &str, kind: EdgeKind) -> Edge {
    Edge {
        from: from.into(),
        to: to.into(),
        kind,
        gap: Duration::zero(),
    }
}

#[test]
fn the_holder_of_a_temporal_relation_is_the_predecessor_and_depends_on_is_the_other_way() {
    // RFC 9253: Task-A (paint) holds FINISHTOSTART towards Task-B (carpet).
    let mut a = task("a", "needs-action");
    a.relate("b", Relation::FinishToStart);
    // `c` depends on `a`: held by the dependent task.
    let mut c = task("c", "needs-action");
    c.relate("a", Relation::DependsOn);
    assert_eq!(
        edges_of(&[a, c]),
        [
            e("a", "b", EdgeKind::FinishToStart),
            e("a", "c", EdgeKind::FinishToStart)
        ]
    );
    for structure in [
        Relation::First,
        Relation::Next,
        Relation::Child,
        Relation::Parent,
    ] {
        assert_eq!(
            Edge::of_relation("a", "b", structure, Duration::zero()),
            None
        );
    }
}

#[test]
fn readiness_follows_the_predecessors() {
    let mut paint = task("paint", "needs-action");
    paint.relate("carpet", Relation::FinishToStart);
    let carpet = task("carpet", "needs-action");
    let r = readiness_of(&[paint.clone(), carpet.clone()]);
    assert_eq!(r["paint"], Readiness::Ready);
    assert_eq!(r["carpet"], Readiness::Blocked(vec!["paint".into()]));

    paint.progress = Some("completed".into());
    let r = readiness_of(&[paint, carpet]);
    assert_eq!(r["paint"], Readiness::Done);
    assert_eq!(r["carpet"], Readiness::Ready);
}

#[test]
fn start_to_start_waits_only_until_the_other_has_started() {
    let mut a = task("a", "in-process");
    a.relate("b", Relation::StartToStart);
    let b = task("b", "needs-action");
    assert_eq!(readiness_of(&[a.clone(), b.clone()])["b"], Readiness::Ready);
    a.progress = Some("needs-action".into());
    assert_eq!(
        readiness_of(&[a, b])["b"],
        Readiness::Blocked(vec!["a".into()])
    );
}

#[test]
fn finish_to_finish_does_not_keep_a_task_from_starting() {
    let mut a = task("a", "needs-action");
    a.relate("b", Relation::FinishToFinish);
    assert_eq!(
        readiness_of(&[a, task("b", "needs-action")])["b"],
        Readiness::Ready
    );
}

#[test]
fn a_cancelled_or_failed_predecessor_never_frees_the_task() {
    for stopped in ["cancelled", "failed"] {
        let mut a = task("a", stopped);
        a.relate("b", Relation::FinishToStart);
        let r = readiness_of(&[a, task("b", "needs-action")]);
        assert_eq!(r["a"], Readiness::Done);
        assert_eq!(r["b"], Readiness::DependencyCancelled(vec!["a".into()]));
    }
}

#[test]
fn an_unknown_dependency_is_named_and_a_known_blocker_wins() {
    let mut b = task("b", "needs-action");
    b.relate("gone", Relation::DependsOn);
    assert_eq!(
        readiness_of(std::slice::from_ref(&b))["b"],
        Readiness::Unknown(vec!["gone".into()])
    );
    b.relate("a", Relation::DependsOn);
    assert_eq!(
        readiness_of(&[b, task("a", "needs-action")])["b"],
        Readiness::Blocked(vec!["a".into()])
    );
}

#[test]
fn cycles_are_found_and_make_their_tasks_a_conflict() {
    let mut a = task("a", "needs-action");
    a.relate("b", Relation::FinishToStart);
    let mut b = task("b", "needs-action");
    b.relate("c", Relation::FinishToStart);
    let mut c = task("c", "needs-action");
    c.relate("a", Relation::FinishToStart);
    let outside = task("d", "needs-action");
    let r = readiness_of(&[a.clone(), b.clone(), c.clone(), outside]);
    let group: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
    assert_eq!(r["a"], Readiness::Conflict(group.clone()));
    assert_eq!(r["c"], Readiness::Conflict(group.clone()));
    assert_eq!(r["d"], Readiness::Ready);
    assert_eq!(cycles(&edges_of(&[a, b, c])), [group]);
}

#[test]
fn a_diamond_is_no_cycle_and_a_self_dependency_is() {
    let diamond = [
        e("a", "b", EdgeKind::FinishToStart),
        e("a", "c", EdgeKind::FinishToStart),
        e("b", "d", EdgeKind::FinishToStart),
        e("c", "d", EdgeKind::FinishToStart),
    ];
    assert!(cycles(&diamond).is_empty());
    assert_eq!(
        cycles(&[e("x", "x", EdgeKind::StartToStart)]),
        [vec!["x".to_string()]]
    );
}

#[test]
fn a_long_chain_does_not_overflow_the_stack() {
    let chain: Vec<Edge> = (0..20_000)
        .map(|i| {
            e(
                &format!("t{i}"),
                &format!("t{}", i + 1),
                EdgeKind::FinishToStart,
            )
        })
        .collect();
    assert!(cycles(&chain).is_empty());
    let closing = e("t20000", "t0", EdgeKind::FinishToStart);
    assert_eq!(path_closing_cycle(&chain, &closing).unwrap().len(), 20_001);
}

#[test]
fn the_path_that_a_new_edge_would_close_is_named() {
    let edges = [
        e("a", "b", EdgeKind::FinishToStart),
        e("b", "c", EdgeKind::FinishToStart),
    ];
    assert_eq!(
        path_closing_cycle(&edges, &e("c", "a", EdgeKind::FinishToStart)),
        Some(vec!["a".to_string(), "b".into(), "c".into()])
    );
    assert_eq!(
        path_closing_cycle(&edges, &e("a", "c", EdgeKind::FinishToStart)),
        None
    );
    assert_eq!(
        path_closing_cycle(&[], &e("z", "z", EdgeKind::FinishToStart)),
        Some(vec!["z".to_string()])
    );
}

#[test]
fn the_gap_of_a_relation_is_read_from_its_member() {
    let mut a = task("a", "needs-action");
    a.relate_with_gap("b", Relation::FinishToStart, "P1D")
        .unwrap();
    a.relate_with_gap("c", Relation::StartToStart, "-PT4H")
        .unwrap();
    let edges = edges_of(&[a.clone()]);
    assert_eq!(edges[0].gap, Duration::days(1));
    assert_eq!(edges[1].gap, Duration::hours(-4));
    assert!(a.relate_with_gap("d", Relation::DependsOn, "P1D").is_err());
    assert!(
        a.relate_with_gap("d", Relation::FinishToStart, "soon")
            .is_err()
    );
    assert!(
        a.relate_with_gap("d", Relation::FinishToStart, "P99999D")
            .is_err()
    );
}
