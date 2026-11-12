//! Shared by the random comparison tests: a small random generator, random tasks (with relations to tasks the store
//! does not hold), and the reference readiness over the whole set (the pure function, judged against all tasks).

#![allow(dead_code, clippy::pedantic)]

use std::collections::BTreeMap;

use fs_tasks_core::plan::readiness_of;
use fs_tasks_core::{Relation, Task, TaskEntry, TaskItems, TaskQuery, TaskRepo};

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

pub const RELS: [Relation; 9] = [
    Relation::DependsOn,
    Relation::FinishToStart,
    Relation::FinishToFinish,
    Relation::StartToStart,
    Relation::StartToFinish,
    Relation::First,
    Relation::Next,
    Relation::Child,
    Relation::Parent,
];
pub const PROG: [Option<&str>; 7] = [
    Some("needs-action"),
    Some("in-process"),
    Some("completed"),
    Some("cancelled"),
    Some("failed"),
    None,
    Some("needs-action"),
];

pub fn random_task(rng: &mut Rng, i: usize, n: usize, ghosts: usize, max_rel: usize) -> Task {
    let uid = format!("u{i}");
    let mut t = Task::new(format!(
        "{}Task {}",
        if rng.below(2) == 0 { "a" } else { "B" },
        rng.below(4)
    ));
    t.uid = uid;
    t.progress = PROG[rng.below(PROG.len())].map(str::to_string);
    if t.progress.is_none() && rng.below(2) == 0 {
        // partial progress through participants
        for k in 0..1 + rng.below(3) {
            let key = t.assign(&format!("p{k}"), &format!("p{k}@x.invalid"));
            let p = ["completed", "in-process", "needs-action", "failed"][rng.below(4)];
            if let Some(part) = t.participants.get_mut(&key) {
                part.extra.insert("progress".into(), serde_json::json!(p));
            }
        }
    }
    if rng.below(3) == 0 {
        t.percent_complete = Some(rng.below(100) as u8);
    }
    for _ in 0..rng.below(max_rel + 1) {
        let other = format!("u{}", rng.below(n + ghosts));
        t.relate(&other, RELS[rng.below(RELS.len())]);
    }
    t
}

/// Whole-graph readiness (the "ideal" of the scan path).
pub fn whole_readiness(all: &[Task], uid: &str) -> String {
    readiness_of(all)
        .remove(uid)
        .map_or_else(|| "Ready".into(), |r| format!("{r:?}"))
}

pub async fn store(r: &TaskRepo, tasks: &[Task]) -> BTreeMap<String, TaskEntry> {
    let mut out = BTreeMap::new();
    for t in tasks {
        let mut t = t.clone();
        t.label("x");
        let e = r.create(t).await.unwrap();
        out.insert(e.task.uid.clone(), e);
    }
    out
}

pub async fn walk_pages(r: &TaskRepo, q: &TaskQuery, size: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut place = None;
    for _ in 0..1000 {
        let p = r.page(q, place.as_ref(), size).await.unwrap();
        assert!(p.items.len() <= size);
        for it in &p.items {
            out.push(it.entry.task.uid.clone());
        }
        if !p.more {
            assert!(p.items.len() <= size);
            return out;
        }
        assert_eq!(p.items.len(), size, "more but not full");
        place = Some(p.items.last().unwrap().place.clone());
    }
    panic!("pages do not end");
}
