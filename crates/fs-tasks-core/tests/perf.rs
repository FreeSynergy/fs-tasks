//! Measurement of the dependency questions on a chain of tasks (task `i` holds a finish-to-start relation to `i + 1`).
//! `FS_TASKS_PERF_N` sets the length (default 300, so the normal run stays short); the numbers of 5000 tasks are in
//! `DESIGN.md`:
//!   `FS_TASKS_PERF_N=5000 cargo test -p fs-tasks-core --test perf -- --nocapture`      (debug)
//!   `FS_TASKS_PERF_N=5000 cargo test --release -p fs-tasks-core --test perf -- --nocapture`

mod common;

use std::time::Instant;

use common::repo;
use fs_tasks_core::plan::Readiness;
use fs_tasks_core::{Relation, Task, TaskItems, TaskQuery};

#[tokio::test]
async fn a_chain_is_judged_as_a_whole_in_batches() {
    let n: usize = std::env::var("FS_TASKS_PERF_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let r = repo().await;
    // The chain goes in as an iCalendar import (linear, as a real import is), the tasks are found again by their uid.
    let mut ics = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\n");
    for i in 0..n {
        ics.push_str(&format!(
            "BEGIN:VTODO\r\nUID:t{i}\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:t{i}\r\nSTATUS:NEEDS-ACTION\r\n"
        ));
        if i + 1 < n {
            ics.push_str(&format!("RELATED-TO;RELTYPE=FINISHTOSTART:t{}\r\n", i + 1));
        }
        ics.push_str("END:VTODO\r\n");
    }
    ics.push_str("END:VCALENDAR\r\n");
    let started = Instant::now();
    let report = r.import_ics(&ics).await.unwrap();
    assert_eq!((report.created.len(), report.failed.len()), (n, 0));
    eprintln!("import {n} tasks:            {:?}", started.elapsed());
    let ids = report.created;
    let started = Instant::now();
    let many = r.readiness_many(&ids).await.unwrap();
    let whole = started.elapsed();
    eprintln!("readiness_many (all {n}):    {whole:?}");
    let by_id: std::collections::BTreeMap<_, _> = many.iter().cloned().collect();
    let head = r.get_by_uid("t0").await.unwrap().unwrap();
    let tail = r.get_by_uid(&format!("t{}", n - 1)).await.unwrap().unwrap();
    assert_eq!(by_id[&head.id], Readiness::Ready, "the head of the chain");
    assert_eq!(
        by_id[&tail.id],
        Readiness::Blocked(vec![format!("t{}", n - 2)]),
        "the end waits for its predecessor"
    );
    assert_eq!(
        many.iter().filter(|(_, r)| *r == Readiness::Ready).count(),
        1
    );

    let started = Instant::now();
    assert_eq!(r.readiness(head.id).await.unwrap(), Readiness::Ready);
    eprintln!("readiness (head, one task):  {:?}", started.elapsed());

    let started = Instant::now();
    let page = r
        .page(&TaskQuery::new().only_ready(true), None, 10)
        .await
        .unwrap();
    let page_time = started.elapsed();
    eprintln!("ready page of 10:            {page_time:?}");
    assert_eq!(page.items.len(), 1);

    let mut ahead = Task::new("front");
    ahead.uid = "front".into();
    let front = r.create(ahead).await.unwrap();
    let started = Instant::now();
    r.relate(front.id, "t0", Relation::FinishToStart)
        .await
        .unwrap();
    let relate_time = started.elapsed();
    eprintln!("relate in front of the head: {relate_time:?}");

    // the way round of a cycle through the whole chain is found too (the quick search gives way to the scan)
    let started = Instant::now();
    let err = r.relate(tail.id, "t0", Relation::FinishToStart).await;
    eprintln!("relate closing a cycle of {n}: {:?}", started.elapsed());
    assert!(matches!(err, Err(fs_tasks_core::Error::Cycle(path)) if path.len() == n));

    // The thresholds of the milestone (release build, 5000 tasks): a `ready` page under 1 s, `relate` in front of a chain
    // under 100 ms, the whole set under 2 s. A debug build is several times slower and gets ten times the room. Before:
    // a chain of 200 took 1.15 s for one readiness and 117 s for a `ready` page.
    let room = if cfg!(debug_assertions) { 10 } else { 1 };
    assert!(
        page_time.as_millis() < 1000 * room * (n as u128).max(1000) / 5000,
        "{page_time:?}"
    );
    assert!(
        relate_time.as_millis() < 100 * room * (n as u128).max(1000) / 1000,
        "{relate_time:?}"
    );
    assert!(
        whole.as_millis() < 2000 * room * (n as u128).max(1000) / 5000,
        "{whole:?}"
    );
}
