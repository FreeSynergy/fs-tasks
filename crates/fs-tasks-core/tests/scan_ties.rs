//! Tasks that share one `published` instant, more of them than one scan chunk (a store written with given times): none is
//! lost, none comes twice, and a full chunk without progress is never mistaken for the end of the scan.

mod common;

use std::sync::Arc;

use common::ME;
use fs_objects::{Query, Store, TursoStore};
use fs_tasks_core::{Task, TaskItems, TaskQuery, TaskRepo};

/// A store of `tied + others` tasks of which the first `tied` share one `published` instant; what the `ready` page, the
/// `ready` list and the plain list find with a scan chunk of `chunk`.
async fn counts(tied: usize, others: usize, chunk: usize) -> (usize, usize, usize) {
    let source: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let plain = TaskRepo::new(source.clone(), ME);
    for i in 0..tied + others {
        let mut t = Task::new(format!("t{i}"));
        t.uid = format!("t{i}");
        plain.create(t).await.unwrap();
    }
    let objects = source.query(&Query::new().of_type("Task")).await.unwrap();
    assert_eq!(objects.len(), tied + others);
    let fixed = objects[0].envelope.published;
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    for (i, mut o) in objects.into_iter().enumerate() {
        if i < tied {
            o.envelope.published = fixed;
        }
        store.put(o, ME).await.unwrap();
    }
    let repo = TaskRepo::new(store, ME).with_scan_chunk(chunk);
    let page = repo
        .page(&TaskQuery::new().only_ready(true), None, 1000)
        .await
        .unwrap();
    let mut uids: Vec<_> = page
        .items
        .iter()
        .map(|p| p.entry.task.uid.clone())
        .collect();
    uids.sort();
    uids.dedup();
    assert_eq!(uids.len(), page.items.len(), "none twice");
    let list = repo
        .query(&TaskQuery::new().only_ready(true))
        .await
        .unwrap()
        .len();
    (page.items.len(), list, repo.list().await.unwrap().len())
}

#[tokio::test]
async fn ties_fewer_than_a_chunk() {
    assert_eq!(counts(5, 0, 7).await, (5, 5, 5));
}

#[tokio::test]
async fn ties_more_than_a_chunk() {
    assert_eq!(counts(5, 0, 2).await, (5, 5, 5));
}

#[tokio::test]
async fn ties_exactly_one_chunk() {
    assert_eq!(counts(5, 0, 5).await, (5, 5, 5));
}

#[tokio::test]
async fn ties_in_the_middle_of_other_tasks_and_a_chunk_that_cuts_them() {
    for chunk in [1, 2, 3, 4, 5, 8, 20] {
        assert_eq!(counts(6, 4, chunk).await, (10, 10, 10), "chunk {chunk}");
    }
}
