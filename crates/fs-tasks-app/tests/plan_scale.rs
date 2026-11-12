//! The plan views on long chains: one reading of the store, pages that never go to the core, a background renewal.
//!
//! Pattern: Black-box tests through `TaskSource` over the real adapter, the real core and the real sample.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use common::World;
use fs_components_workbenches::tasks::{RowQuery, TaskRow, TaskSource};
use fs_tasks_app::plan_view::{Board, Cache, Next, PlanSource};
use fs_tasks_app::{Day, sample_chain};
use fs_tasks_core::TaskItems;

const TODAY: &str = "2026-11-12";

fn today() -> Day {
    Day::parse(TODAY).unwrap()
}

fn world_with(ics: &str) -> World {
    let w = World::new();
    let report = w.runtime.block_on(w.repo.import_ics(ics)).unwrap();
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    w
}

fn source(w: &World, layout: Box<dyn fs_tasks_app::plan_view::Layout>) -> (PlanSource, Arc<Cache>) {
    let items: Arc<dyn TaskItems> = w.repo.clone();
    let cache = Cache::start(items, w.runtime.handle().clone(), today());
    cache.wait(Duration::from_secs(120)).expect("a picture");
    (
        PlanSource::new(w.adapter.clone(), cache.clone(), layout),
        cache,
    )
}

fn all(s: &PlanSource) -> Vec<TaskRow> {
    s.rows(&RowQuery::default(), 0, 10_000).unwrap()
}

/// The titles under each heading of the "what next" list, in order.
fn groups(rows: &[TaskRow]) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for row in rows {
        if row.key.starts_with("group:") {
            out.push((row.title.clone(), Vec::new()));
        } else {
            out.last_mut()
                .expect("a heading first")
                .1
                .push(row.title.clone());
        }
    }
    out
}

#[test]
fn a_long_chain_is_read_once_and_only_its_head_is_ready() {
    let n: usize = std::env::var("FS_TASKS_APP_CHAIN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1500);
    let w = world_with(&sample_chain(n, today()));
    let started = Instant::now();
    let (s, cache) = source(&w, Box::new(Next));
    let loaded = started.elapsed();
    let t = Instant::now();
    let rows = s.rows(&RowQuery::default(), 0, 200).unwrap();
    let draw = t.elapsed();
    eprintln!("chain {n}: picture {loaded:?}, one page of the list {draw:?}");
    let g = groups(&all(&s));
    assert_eq!(g.iter().map(|(_, t)| t.len()).sum::<usize>(), n);
    let ready: usize = g
        .iter()
        .filter(|(h, _)| !h.starts_with("BLOCKED"))
        .map(|(_, t)| t.len())
        .sum();
    // the head, and every 97th step that was started... which still waits for its predecessor
    assert_eq!(ready, 1, "only the head can start");
    assert_eq!(rows.len(), 200);
    // reading a page never goes to the core
    let before = cache.loads();
    for _ in 0..20 {
        s.rows(&RowQuery::default(), 0, 200).unwrap();
    }
    assert_eq!(cache.loads(), before);
}

#[test]
fn a_big_store_is_renewed_in_the_background_and_the_old_picture_serves_meanwhile() {
    let n = 700; // above the size that is renewed at once
    let w = world_with(&sample_chain(n, today()));
    let (s, cache) = source(&w, Box::new(Board));
    let told = Arc::new(AtomicUsize::new(0));
    let t = told.clone();
    cache.on_change(move || {
        t.fetch_add(1, Ordering::SeqCst);
    });
    let old = cache.current().unwrap();
    let mut draft = s.blank().unwrap();
    draft.fields[0].value = "One more".into();
    s.create(&draft).unwrap();
    assert!(
        Arc::ptr_eq(&old, &cache.current().unwrap()),
        "the page is drawn from the old picture"
    );
    assert_eq!(s.rows(&RowQuery::default(), 0, 5).unwrap().len(), 5);
    let until = Instant::now() + Duration::from_secs(120);
    while told.load(Ordering::SeqCst) == 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(told.load(Ordering::SeqCst), 1, "the window is told once");
    assert_eq!(cache.current().unwrap().entries.len(), n + 1);
}
