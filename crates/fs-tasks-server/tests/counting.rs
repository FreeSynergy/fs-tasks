//! Reading all pages must cost a number of rows that grows with the data, not with its square.
//! The count is of rows the store returns, not of seconds: a fixed time says nothing on another
//! machine. The store is behind a counting decorator under the provider.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use common::{ME, Read, items_of, read_pages, with_tasks_server};
use fs_store::traits::{ObjectQuery, ObjectRepository};
use fs_store::{Activity, LogEntry, ObjectId, Query, Result, Store, StoredObject, TursoStore};
use fs_tasks_core::{Task, TaskItems, TaskRepo};
use fs_tasks_server::TasksServer;

/// Passes everything on and counts the objects the queries hand back.
struct Counting {
    store: Arc<dyn Store>,
    rows: AtomicUsize,
}

#[async_trait]
impl ObjectRepository for Counting {
    async fn put(&self, object: StoredObject, actor: &str) -> Result<Activity> {
        self.store.put(object, actor).await
    }
    async fn put_and_log(
        &self,
        object: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> Result<Vec<Activity>> {
        self.store.put_and_log(object, actor, extra).await
    }
    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        self.store.get(id).await
    }
    async fn delete(&self, id: ObjectId, actor: &str) -> Result<Activity> {
        self.store.delete(id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for Counting {
    async fn query(&self, query: &Query) -> Result<Vec<StoredObject>> {
        let found = self.store.query(query).await?;
        self.rows.fetch_add(found.len(), Ordering::Relaxed);
        Ok(found)
    }
    async fn count(&self, query: &Query) -> Result<usize> {
        self.store.count(query).await
    }
}

const PAGE: usize = 8;

/// Rows read to page through `n` tasks with `how`.
async fn rows_for(n: usize, how: Read) -> (usize, usize) {
    let counting = Arc::new(Counting {
        store: Arc::new(TursoStore::open_in_memory().await.unwrap()),
        rows: AtomicUsize::new(0),
    });
    let repo = Arc::new(TaskRepo::with_backend(
        counting.clone(),
        counting.clone(),
        ME,
    ));
    for i in 0..n {
        let mut task = Task::new(format!("Task {}", (i * 7919) % n));
        task.uid = format!("uid-{i:05}");
        task.due = Some(format!("2026-12-{:02}T10:00:00", 1 + i % 28));
        task.time_zone = Some("Europe/Berlin".into());
        repo.create(task).await.unwrap();
    }
    counting.rows.store(0, Ordering::Relaxed);
    let server = TasksServer::new(repo.clone(), repo).with_page_size(PAGE);
    let seen = std::rc::Rc::new(std::cell::Cell::new(0));
    let slot = seen.clone();
    with_tasks_server(server, |root| async move {
        let items = items_of(&root).await;
        slot.set(read_pages(&items, how, |_| async {}).await.len());
    })
    .await;
    (counting.rows.load(Ordering::Relaxed), seen.get())
}

#[tokio::test(flavor = "current_thread")]
async fn reading_all_pages_costs_rows_in_proportion_to_the_data() {
    for (name, how) in [("list", Read::List), ("query", Read::Query(""))] {
        let (small, seen_small) = rows_for(80, how).await;
        let (large, seen_large) = rows_for(160, how).await;
        assert_eq!(
            (seen_small, seen_large),
            (80, 160),
            "{name}: every task once"
        );
        #[allow(clippy::cast_precision_loss)]
        let ratio = large as f64 / small as f64;
        eprintln!(
            "{name}: {small} rows for 80 tasks, {large} rows for 160 tasks, ratio {ratio:.2}"
        );
        // Linear is about 2, a whole list read for every page is about 4.
        assert!(
            ratio < 3.0,
            "{name}: {small} -> {large} rows, ratio {ratio:.2}"
        );
        assert!(
            large < 2 * 160 + 10 * 160 / PAGE,
            "{name}: {large} rows for 160 tasks"
        );
    }
}
