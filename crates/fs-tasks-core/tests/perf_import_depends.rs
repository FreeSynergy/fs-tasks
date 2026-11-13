//! Importing a long `DEPENDS-ON` chain is one judgement of the whole graph, not one scan for each task a relation names
//! (that was quadratic: a chain of 1500 did not finish in a debug build). The right tasks are announced, once each.
//! `FS_TASKS_PERF_N` sets the length (default 300); a debug build gets twenty seconds for it.

mod common;

use std::fmt::Write;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use common::repo;
use fs_tasks_core::{Relation, Task, TaskChange, TaskFeed, TaskItems, TaskListener};

struct Log(Mutex<Vec<TaskChange>>);
impl TaskListener for Log {
    fn changed(&self, c: &TaskChange) {
        self.0.lock().unwrap().push(*c);
    }
}

#[tokio::test]
async fn importing_a_depends_on_chain_is_not_quadratic_and_announces_the_right_tasks() {
    let n: usize = std::env::var("FS_TASKS_PERF_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if cfg!(debug_assertions) { 300 } else { 5000 });
    let r = repo().await;
    // `w` waits for the end of the chain, which is not there yet
    let mut w = Task::new("w");
    w.uid = "w".into();
    w.relate(&format!("t{}", n - 1), Relation::DependsOn);
    let w = r.create(w).await.unwrap();
    let log = Arc::new(Log(Mutex::default()));
    let _sub = r.subscribe(log.clone());
    // t0 depends on t1 ... on t{n-1}, which is completed: only t{n-2} (and `w`) are ready now that it is there
    let mut ics = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\n");
    for i in 0..n {
        let status = if i + 1 == n {
            "COMPLETED"
        } else {
            "NEEDS-ACTION"
        };
        write!(
            ics,
            "BEGIN:VTODO\r\nUID:t{i}\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:t{i}\r\nSTATUS:{status}\r\n"
        )
        .unwrap();
        if i + 1 < n {
            write!(ics, "RELATED-TO;RELTYPE=DEPENDS-ON:t{}\r\n", i + 1).unwrap();
        }
        ics.push_str("END:VTODO\r\n");
    }
    ics.push_str("END:VCALENDAR\r\n");
    let started = Instant::now();
    let report = r.import_ics(&ics).await.unwrap();
    let took = started.elapsed();
    eprintln!("import of a DEPENDS-ON chain of {n}: {took:?}");
    assert_eq!((report.created.len(), report.failed.len()), (n, 0));
    let before_end = r.get_by_uid(&format!("t{}", n - 2)).await.unwrap().unwrap();
    let mut got: Vec<_> = log
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|c| match c {
            TaskChange::Unblocked(id) => Some(*id),
            _ => None,
        })
        .collect();
    let mut want = vec![w.id, before_end.id];
    got.sort();
    want.sort();
    assert_eq!(got, want, "exactly the freed tasks, each once");
    let room = if cfg!(debug_assertions) { 20_000 } else { 5_000 };
    assert!(took.as_millis() < room, "{took:?}");
}
