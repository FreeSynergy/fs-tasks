//! Schedule: earliest and latest start, slack, gaps, and no panic on dates out of range.

use chrono::{DateTime, Duration, TimeZone, Utc};
use fs_tasks_core::plan::{Edge, EdgeKind, Item, ScheduleError, schedule};

fn e(from: &str, to: &str, kind: EdgeKind) -> Edge {
    Edge {
        from: from.into(),
        to: to.into(),
        kind,
        gap: Duration::zero(),
    }
}

fn at(hours: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap() + Duration::hours(hours)
}

fn item(uid: &str, hours: i64, fixed: Option<i64>) -> Item {
    Item {
        uid: uid.into(),
        duration: Duration::hours(hours),
        fixed_start: fixed.map(at),
    }
}

#[test]
fn schedule_places_a_chain_and_finds_the_slack() {
    // a(2h) -> b(3h) -> d(1h); a -> c(1h) -> d: c has 2h of slack.
    let items = [
        item("a", 2, Some(0)),
        item("b", 3, None),
        item("c", 1, None),
        item("d", 1, None),
    ];
    let edges = [
        e("a", "b", EdgeKind::FinishToStart),
        e("a", "c", EdgeKind::FinishToStart),
        e("b", "d", EdgeKind::FinishToStart),
        e("c", "d", EdgeKind::FinishToStart),
    ];
    let plan = schedule(&items, &edges).unwrap();
    let get = |u: &str| plan.iter().find(|p| p.uid == u).unwrap().clone();
    assert_eq!((get("b").start, get("b").end), (at(2), at(5)));
    assert_eq!(get("d").start, at(5));
    assert_eq!(get("b").slack, Duration::zero());
    assert_eq!(get("c").slack, Duration::hours(2));
    assert_eq!(get("d").end, at(6));
}

#[test]
fn schedule_knows_the_other_three_ties() {
    let items = [item("a", 4, Some(0)), item("b", 2, None)];
    let start = |kind| {
        let plan = schedule(&items, &[e("a", "b", kind)]).unwrap();
        plan.iter().find(|p| p.uid == "b").unwrap().start
    };
    assert_eq!(start(EdgeKind::StartToStart), at(0));
    assert_eq!(start(EdgeKind::FinishToFinish), at(2));
    assert_eq!(start(EdgeKind::StartToFinish), at(-2));
}

#[test]
fn schedule_does_not_place_what_has_no_anchor_and_refuses_cycles() {
    let items = [item("a", 1, None), item("b", 1, None)];
    assert_eq!(
        schedule(&items, &[e("a", "b", EdgeKind::FinishToStart)]).unwrap(),
        []
    );
    assert_eq!(
        schedule(
            &items,
            &[
                e("a", "b", EdgeKind::FinishToStart),
                e("b", "a", EdgeKind::FinishToStart)
            ]
        ),
        Err(ScheduleError::Cycle(vec!["a".into(), "b".into()]))
    );
}

#[test]
fn a_gap_moves_the_successor_and_a_lead_pulls_it_forward() {
    let items = [item("a", 4, Some(0)), item("b", 2, None)];
    let with = |gap_hours: i64| {
        let mut edge = e("a", "b", EdgeKind::FinishToStart);
        edge.gap = Duration::hours(gap_hours);
        let plan = schedule(&items, &[edge]).unwrap();
        plan.iter().find(|p| p.uid == "b").unwrap().start
    };
    assert_eq!(with(0), at(4));
    assert_eq!(with(24), at(28), "lag of one day");
    assert_eq!(with(-1), at(3), "lead of one hour");
}

#[test]
fn durations_that_leave_the_range_of_dates_are_an_error_not_a_panic() {
    let huge = Duration::days(100_000_000);
    let items = [
        Item {
            uid: "a".into(),
            duration: huge,
            fixed_start: Some(at(0)),
        },
        Item {
            uid: "b".into(),
            duration: huge,
            fixed_start: None,
        },
    ];
    assert_eq!(
        schedule(&items, &[e("a", "b", EdgeKind::FinishToStart)]),
        Err(ScheduleError::OutOfRange)
    );
}
