//! An import costs work in proportion to its size, not to its square. What is counted is what the
//! provider asks of the store (calls and rows), never seconds: a fixed time says nothing on another
//! machine.
//!
//! The store cannot say how much it looks at inside one query. The measured fact (a lookup by an index
//! field took 22 µs per stored task in a debug build, 1 000 lookups over 1 000 tasks 1.1 s) is that
//! a query with an index or an OR condition looks at every task of its type, whatever it returns. So
//! the decorator below charges such a query the number of tasks stored at that moment, on top of the
//! rows it returns. An import that asks the store for each of its tasks whether the `uid` is known is
//! charged n times a growing number: n and 2n tasks cost 1 : 4. One that reads the known `uid`s once
//! costs 1 : 2.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use common::ME;
use fs_objects::traits::{ObjectQuery, ObjectRepository};
use fs_objects::{Activity, LogEntry, ObjectId, Query, Result, Store, StoredObject, TursoStore};
use fs_tasks_core::{TaskItems, TaskRepo};

/// Passes everything on and counts the work the provider asks for.
struct Charging {
    store: Arc<dyn Store>,
    /// Calls of `get`, `put` and `query`, plus the rows the queries hand back and what a lookup by an
    /// index field is charged.
    work: AtomicUsize,
    /// Queries that look for a task by an index field or an OR group.
    lookups: AtomicUsize,
}

#[async_trait]
impl ObjectRepository for Charging {
    async fn put(&self, object: StoredObject, actor: &str) -> Result<Activity> {
        self.work.fetch_add(1, Ordering::Relaxed);
        self.store.put(object, actor).await
    }
    async fn put_and_log(
        &self,
        object: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> Result<Vec<Activity>> {
        self.work.fetch_add(1, Ordering::Relaxed);
        self.store.put_and_log(object, actor, extra).await
    }
    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        self.work.fetch_add(1, Ordering::Relaxed);
        self.store.get(id).await
    }
    async fn delete(&self, id: ObjectId, actor: &str) -> Result<Activity> {
        self.store.delete(id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for Charging {
    async fn query(&self, query: &Query) -> Result<Vec<StoredObject>> {
        let found = self.store.query(query).await?;
        let mut cost = 1 + found.len();
        if !query.index.is_empty() || !query.conds.is_empty() || !query.any.is_empty() {
            self.lookups.fetch_add(1, Ordering::Relaxed);
            cost += self.store.count(&Query::new().of_type("Task")).await?;
        }
        self.work.fetch_add(cost, Ordering::Relaxed);
        Ok(found)
    }
    async fn count(&self, query: &Query) -> Result<usize> {
        self.store.count(query).await
    }
}

fn ics(first: usize, n: usize) -> String {
    let mut s = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\n");
    for i in first..first + n {
        s += &format!(
            "BEGIN:VTODO\r\nUID:todo-{i:05}\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261112T170000Z\r\nSUMMARY:T{i}\r\nEND:VTODO\r\n"
        );
    }
    s + "END:VCALENDAR\r\n"
}

/// Work and index lookups to import `n` new tasks into a store that holds `before` others.
async fn work_for(before: usize, n: usize) -> (usize, usize) {
    let charging = Arc::new(Charging {
        store: Arc::new(TursoStore::open_in_memory().await.unwrap()),
        work: AtomicUsize::new(0),
        lookups: AtomicUsize::new(0),
    });
    let repo = TaskRepo::with_backend(charging.clone(), charging.clone(), ME);
    let first = repo.import_ics(&ics(10_000, before)).await.unwrap();
    assert_eq!(first.created.len(), before);
    charging.work.store(0, Ordering::Relaxed);
    charging.lookups.store(0, Ordering::Relaxed);
    let report = repo.import_ics(&ics(0, n)).await.unwrap();
    assert_eq!(
        (report.created.len(), report.failed.len()),
        (n, 0),
        "{report:?}"
    );
    (
        charging.work.load(Ordering::Relaxed),
        charging.lookups.load(Ordering::Relaxed),
    )
}

#[tokio::test]
async fn importing_twice_the_tasks_costs_twice_the_work() {
    for before in [0, 40] {
        let (small, lookups_small) = work_for(before, 60).await;
        let (large, lookups_large) = work_for(before, 120).await;
        #[allow(clippy::cast_precision_loss)]
        let ratio = large as f64 / small as f64;
        eprintln!(
            "{before} stored: 60 tasks {small} (lookups {lookups_small}), 120 tasks {large} (lookups {lookups_large}), ratio {ratio:.2}"
        );
        assert!(
            ratio < 2.5,
            "{before} stored: {small} -> {large}, ratio {ratio:.2}"
        );
        // Linear: the same few calls for each task, whatever the store holds.
        assert!(large < 10 * 120, "{before} stored: {large} for 120 tasks");
        // The known uids are read once, in pages: a lookup does not grow with the number of tasks.
        assert!(
            lookups_large <= lookups_small + 2,
            "{before} stored: {lookups_small} -> {lookups_large} lookups"
        );
    }
}

#[tokio::test]
async fn known_uids_are_skipped_also_when_they_come_twice_in_one_text() {
    let charging = Arc::new(Charging {
        store: Arc::new(TursoStore::open_in_memory().await.unwrap()),
        work: AtomicUsize::new(0),
        lookups: AtomicUsize::new(0),
    });
    let repo = TaskRepo::with_backend(charging.clone(), charging, ME);
    assert_eq!(repo.import_ics(&ics(0, 3)).await.unwrap().created.len(), 3);
    // todo-00001 and todo-00002 are stored; 3 and 4 are new, and 3 is in the text twice.
    let text = format!("{}{}", ics(1, 4), ics(3, 1));
    let report = repo.import_ics(&text).await.unwrap();
    assert_eq!(report.created.len(), 2, "{report:?}");
    assert_eq!(report.skipped.len(), 3, "{report:?}");
    assert_eq!(repo.list().await.unwrap().len(), 5);
}
