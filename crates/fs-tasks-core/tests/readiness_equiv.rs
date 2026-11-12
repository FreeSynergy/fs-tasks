#![allow(clippy::pedantic)]
//! The same readiness by every way of asking: one task, many tasks, the `ready` list and its pages (scan path and query
//! path), against the pure function over all stored tasks, on random graphs with relations to tasks the store does not
//! hold. A task that is not held is a node without relations of its own whose dependents count, so a cycle through it
//! is a conflict however it is asked. Also: the pure function equals the v0.0.5 copy in `reference/`.

mod common;
#[allow(dead_code, clippy::pedantic)]
mod reference;

use std::collections::{BTreeMap, BTreeSet};

use common::random::{Rng, random_task, store, walk_pages, whole_readiness};
use common::repo;
use fs_tasks_core::plan::readiness_of;
use fs_tasks_core::{Task, TaskItems, TaskQuery};

async fn equiv_runs(seed: u64, runs: usize) -> (usize, usize, Vec<String>) {
    let mut rng = Rng(seed);
    let mut checked = 0;
    let mut findings = Vec::new();
    for run in 0..runs {
        let n = if rng.below(10) == 0 {
            21 + rng.below(6)
        } else {
            1 + rng.below(9)
        };
        let ghosts = rng.below(3);
        let tasks: Vec<Task> = (0..n)
            .map(|i| random_task(&mut rng, i, n, ghosts, 3))
            .collect();
        let r = repo().await;
        let stored = store(&r, &tasks).await;
        let all: Vec<Task> = stored.values().map(|e| e.task.clone()).collect();
        let ids: Vec<_> = stored.values().map(|e| e.id).collect();
        let many: BTreeMap<_, _> = r.readiness_many(&ids).await.unwrap().into_iter().collect();
        let mut ready_ref = BTreeSet::new();
        let mut not_ready_ref = BTreeSet::new();
        for (uid, e) in &stored {
            let old = whole_readiness(&all, uid);
            let one = format!("{:?}", r.readiness(e.id).await.unwrap());
            let m = format!("{:?}", many[&e.id]);
            checked += 1;
            if (old != one || old != m) && findings.len() < 12 {
                findings.push(format!(
                    "seed {seed} run {run} n {n} uid {uid}: whole {old} single {one} many {m}; tasks: {:?}",
                    all.iter()
                        .map(|t| (t.uid.clone(), t.effective_progress().to_string(), t.relations()))
                        .collect::<Vec<_>>()
                ));
            }
            if old == "Ready" {
                ready_ref.insert(uid.clone());
            } else {
                not_ready_ref.insert(uid.clone());
            }
        }
        // list and pages, scan path (no other filter) and query path (label)
        for (wanted, reference) in [(true, &ready_ref), (false, &not_ready_ref)] {
            for q in [
                TaskQuery::new().only_ready(wanted),
                TaskQuery::new().only_ready(wanted).with_label("x"),
            ] {
                let listed: BTreeSet<String> = r
                    .query(&q)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|e| e.task.uid)
                    .collect();
                let size = 1 + rng.below(4);
                let paged = walk_pages(&r, &q, size).await;
                let paged_set: BTreeSet<String> = paged.iter().cloned().collect();
                let ok = &listed == reference
                    && &paged_set == reference
                    && paged.len() == paged_set.len();
                if !ok && findings.len() < 12 {
                    findings.push(format!(
                        "[LIST] seed {seed} run {run} wanted {wanted} label {:?} size {size}: ref {reference:?} list {listed:?} pages {paged:?}",
                        q.label
                    ));
                }
            }
        }
    }
    (checked, 0, findings)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_way_of_asking_gives_the_same_readiness() {
    let runs: usize = std::env::var("REV_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400);
    let parts = 8;
    let mut handles = Vec::new();
    for p in 0..parts {
        handles.push(tokio::spawn(equiv_runs(
            0x9E37_79B9_7F4A_7C15 ^ ((p as u64 + 1) * 7919),
            runs / parts,
        )));
    }
    let (mut checked, mut ghosts, mut findings) = (0, 0, Vec::new());
    for h in handles {
        let (c, g, f) = h.await.unwrap();
        checked += c;
        ghosts += g;
        findings.extend(f);
    }
    eprintln!(
        "EQUIV runs {runs} tasks checked {checked}, ghost-only diffs {ghosts}, findings {}",
        findings.len()
    );
    for f in &findings {
        eprintln!("{f}");
    }
    assert!(findings.is_empty());
}

/// The pure function after this release's change (edges by their successor, `satisfied_by`) equals the one of 0.0.5.
#[test]
fn the_pure_readiness_equals_the_one_of_0_0_5() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    for _ in 0..3000 {
        let n = 1 + rng.below(12);
        let ghosts = rng.below(3);
        let tasks: Vec<_> = (0..n)
            .map(|i| random_task(&mut rng, i, n, ghosts, 3))
            .collect();
        let new: BTreeMap<String, String> = readiness_of(&tasks)
            .into_iter()
            .map(|(u, r)| (u, format!("{r:?}")))
            .collect();
        let old: BTreeMap<String, String> = reference::readiness_of(&tasks)
            .into_iter()
            .map(|(u, r)| (u, format!("{r:?}")))
            .collect();
        assert_eq!(new, old);
    }
}
