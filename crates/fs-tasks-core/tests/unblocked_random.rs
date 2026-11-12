#![allow(clippy::pedantic)]
//! `Unblocked` against the expectation "ready now and not ready before" on random graphs, for seven kinds of change: a
//! progress, a relation taken away, a deleted task, the KIND of a relation changed, a relation added, the progress that
//! comes from the participants, and the creation of a task that a relation already named (also a cycle that is resolved,
//! and no second announcement for a task that was ready already).

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use common::random::{Rng, random_task, store, whole_readiness};
use common::repo;
use fs_tasks_core::plan::{cycles, edges_of};
use fs_tasks_core::{Task, TaskChange, TaskFeed, TaskItems, TaskListener};

struct Log(Mutex<Vec<TaskChange>>);
impl TaskListener for Log {
    fn changed(&self, change: &TaskChange) {
        self.0.lock().unwrap().push(*change);
    }
}

async fn unblocked_runs(seed: u64, runs: usize) -> Vec<String> {
    let mut rng = Rng(seed);
    let mut findings = Vec::new();
    for run in 0..runs {
        let n = 2 + rng.below(6);
        let ghosts = rng.below(2);
        let tasks: Vec<Task> = (0..n)
            .map(|i| random_task(&mut rng, i, n, ghosts, 2))
            .collect();
        let r = repo().await;
        let stored = store(&r, &tasks).await;
        let all: Vec<Task> = stored.values().map(|e| e.task.clone()).collect();
        let before: BTreeMap<String, String> = stored
            .keys()
            .map(|u| (u.clone(), whole_readiness(&all, u)))
            .collect();
        let log = Arc::new(Log(Mutex::default()));
        let _sub = r.subscribe(log.clone());
        let target = format!("u{}", rng.below(n));
        let te = &stored[&target];
        let op = rng.below(7);
        let opname;
        match op {
            0 => {
                let mut t = te.task.clone();
                t.progress = Some(
                    [
                        "needs-action",
                        "in-process",
                        "completed",
                        "cancelled",
                        "failed",
                    ][rng.below(5)]
                    .into(),
                );
                opname = format!(
                    "progress {:?}->{:?}",
                    te.task.effective_progress(),
                    t.progress
                );
                r.edit(te.id, t).await.unwrap();
            }
            1 => {
                let mut t = te.task.clone();
                let keys: Vec<String> = t.related_to.keys().cloned().collect();
                if keys.is_empty() {
                    opname = "noop".into();
                } else {
                    let k = &keys[rng.below(keys.len())];
                    t.related_to.remove(k);
                    opname = format!("remove relation to {k}");
                    r.edit(te.id, t).await.unwrap();
                }
            }
            2 => {
                opname = "delete".into();
                r.delete(te.id).await.unwrap();
            }
            3 => {
                let mut t = te.task.clone();
                let keys: Vec<String> = t.related_to.keys().cloned().collect();
                if keys.is_empty() {
                    opname = "noop".into();
                } else {
                    let k = keys[rng.below(keys.len())].clone();
                    t.related_to.remove(&k);
                    let rel = common::random::RELS[rng.below(9)];
                    t.relate(&k, rel);
                    opname = format!(
                        "KIND relation to {k} -> {rel:?} (was {:?})",
                        te.task.relations()
                    );
                    r.edit(te.id, t).await.unwrap();
                }
            }
            4 => {
                let mut t = te.task.clone();
                let other = format!("u{}", rng.below(n + ghosts));
                let rel = common::random::RELS[rng.below(9)];
                t.relate(&other, rel);
                opname = format!("ADD relation {other} {rel:?}");
                r.edit(te.id, t).await.unwrap();
            }
            5 => {
                let mut t = te.task.clone();
                t.progress = None;
                t.participants.clear();
                for k in 0..1 + rng.below(3) {
                    let key = t.assign(&format!("p{k}"), &format!("p{k}@x.invalid"));
                    let p = ["completed", "in-process", "needs-action", "failed"][rng.below(4)];
                    if let Some(part) = t.participants.get_mut(&key) {
                        part.extra.insert("progress".into(), serde_json::json!(p));
                    }
                }
                opname = format!(
                    "PARTICIPANTS {:?}->{:?}",
                    te.task.effective_progress(),
                    t.effective_progress()
                );
                r.edit(te.id, t).await.unwrap();
            }
            _ => {
                if ghosts == 0 {
                    opname = "noop".into();
                } else {
                    let mut t = Task::new("ghost".to_string());
                    t.uid = format!("u{n}");
                    t.progress = common::random::PROG[rng.below(7)].map(str::to_string);
                    opname = format!("CREATE ghost u{n} {:?}", t.progress);
                    r.create(t).await.unwrap();
                }
            }
        }
        let after_tasks: Vec<Task> = r
            .list()
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.task)
            .collect();
        let after: BTreeMap<String, String> = after_tasks
            .iter()
            .map(|t| (t.uid.clone(), whole_readiness(&after_tasks, &t.uid)))
            .collect();
        let mut expected: BTreeSet<String> = BTreeSet::new();
        for (u, a) in &after {
            let Some(b) = before.get(u) else { continue };
            if a == "Ready" && b != "Ready" && !(u == &target && b == "Done") {
                expected.insert(u.clone());
            }
        }
        let by_id: BTreeMap<_, _> = stored
            .values()
            .map(|e| (e.id, e.task.uid.clone()))
            .collect();
        let got: Vec<String> = log
            .0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|c| match c {
                TaskChange::Unblocked(id) => {
                    Some(by_id.get(id).cloned().unwrap_or_else(|| "NEW".into()))
                }
                _ => None,
            })
            .collect();
        let got_set: BTreeSet<String> = got.iter().cloned().collect();
        if got_set != expected || got.len() != got_set.len() {
            let cyc = cycles(&edges_of(&all));
            if findings.len() < 400 {
                findings.push(format!(
                    "[UNBLOCKED] seed {seed} run {run} op {opname} on {target}: expected {expected:?} got {got:?}; before {before:?} after {after:?} cycles {cyc:?}"
                ));
            }
        }
    }
    findings
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unblocked_matches_the_expectation_for_every_kind_of_change() {
    let runs: usize = std::env::var("REV_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400);
    let parts = 8;
    let mut handles = Vec::new();
    for p in 0..parts {
        handles.push(tokio::spawn(unblocked_runs(
            0xA5A5_1234_9876_0001 ^ ((p as u64 + 5) * 15_485_863),
            runs / parts,
        )));
    }
    let mut findings = Vec::new();
    for h in handles {
        findings.extend(h.await.unwrap());
    }
    eprintln!("UNBLOCKED runs {runs}: findings {}", findings.len());
    for f in findings.iter() {
        eprintln!("{f}");
    }
    assert!(findings.is_empty());
}
