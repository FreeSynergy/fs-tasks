//! Helpers for the paging and filter tests: tasks with optional parts, and every page of a read.

#![allow(dead_code)] // each test file uses a different subset

use std::sync::Arc;

use fs_interfaces::rpc::read_envelope;
use fs_interfaces::tasks_capnp::{self, items, tasks};
use fs_store::{Store, TursoStore};
use fs_tasks_core::TaskRepo;
use fs_tasks_server::TasksServer;
use uuid::Uuid;

use super::{ME, task_json, with_tasks_server};

/// A task with the optional parts a filter looks at. `due` is a LocalDateTime in Berlin time.
pub fn json_with(
    uid: &str,
    title: &str,
    description: Option<&str>,
    due: Option<&str>,
    progress: Option<&str>,
) -> String {
    let mut task = serde_json::json!({
        "@type": "Task", "version": "2.0", "uid": uid, "title": title, "timeZone": "Europe/Berlin",
    });
    for (key, value) in [
        ("description", description),
        ("due", due),
        ("progress", progress),
    ] {
        if let Some(value) = value {
            task[key] = value.into();
        }
    }
    task.to_string()
}

/// Create a task from its JSON text and answer its object id.
pub async fn create_text(items: &items::Client, text: &str) -> Uuid {
    let mut req = items.create_request();
    req.get().set_jscalendar(text);
    let answer = req.send().promise.await.unwrap();
    let task = answer.get().unwrap().get_task().unwrap();
    read_envelope(task.get_envelope().unwrap())
        .unwrap()
        .id
        .as_uuid()
}

/// Which call of the contract reads the pages.
#[derive(Clone, Copy)]
pub enum Read {
    /// `Items.list`.
    List,
    /// `Items.query` with this label (empty = no filter at all).
    Query(&'static str),
}

/// Every uid of every page, in the order the server gave them, after `between` ran once after
/// the first page (a hook for a test that changes the data between two pages).
pub async fn read_pages<F, Fut>(items: &items::Client, how: Read, mut between: F) -> Vec<String>
where
    F: FnMut(Vec<Uuid>) -> Fut,
    Fut: Future<Output = ()>,
{
    let (mut uids, mut cursor, mut first) = (Vec::new(), Vec::new(), true);
    loop {
        let (page_uids, ids, next) = match how {
            Read::List => {
                let mut call = items.list_request();
                call.get().init_cursor().set_opaque(&cursor);
                let answer = call.send().promise.await.unwrap();
                page_of(answer.get().unwrap().get_page().unwrap())
            }
            Read::Query(label) => {
                let mut call = items.query_request();
                call.get().init_filter().set_label(label);
                call.get().init_cursor().set_opaque(&cursor);
                let answer = call.send().promise.await.unwrap();
                page_of(answer.get().unwrap().get_page().unwrap())
            }
        };
        uids.extend(page_uids);
        if std::mem::take(&mut first) {
            between(ids).await;
        }
        if next.is_empty() {
            return uids;
        }
        cursor = next;
    }
}

fn page_of(page: tasks_capnp::task_page::Reader<'_>) -> (Vec<String>, Vec<Uuid>, Vec<u8>) {
    let tasks: Vec<_> = page.get_items().unwrap().iter().collect();
    let uids = tasks
        .iter()
        .map(|t| task_json(*t)["uid"].as_str().unwrap().to_string())
        .collect();
    let ids = tasks
        .iter()
        .map(|t| {
            read_envelope(t.get_envelope().unwrap())
                .unwrap()
                .id
                .as_uuid()
        })
        .collect();
    let next = page.get_next().unwrap().get_opaque().unwrap().to_vec();
    (uids, ids, next)
}

/// A server that cuts its pages at `size` items.
pub async fn with_page_size<F, Fut>(size: usize, test: F)
where
    F: FnOnce(tasks::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let repo = Arc::new(TaskRepo::new(store, ME));
    with_tasks_server(
        TasksServer::new(repo.clone(), repo).with_page_size(size),
        test,
    )
    .await;
}
