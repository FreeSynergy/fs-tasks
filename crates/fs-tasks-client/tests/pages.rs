//! `TaskPages` and `PageWalk`: the list read page by page, by cursor, against the real server and
//! the real store.
//!
//! Integration tests against the trait; no stand-ins.

mod common;

use std::sync::Arc;

use common::{Served, task};
use fs_store::ObjectId;
use fs_tasks_client::{Cursor, PageWalk, RevisedTasks, RpcTasks, TaskPages};
use fs_tasks_core::{Error, TaskItems, TaskQuery};

/// Task `n` is due `n` minutes after midnight of 1 December 2026, so the order is the number.
fn timed(n: usize) -> fs_tasks_core::Task {
    let mut t = task(&format!("uid-{n:04}"), &format!("T {n:04}"));
    t.due = Some(format!("2026-12-01T{:02}:{:02}:00", n / 60, n % 60));
    t
}

async fn fill(tasks: &RpcTasks, count: usize) -> Vec<ObjectId> {
    let mut ids = Vec::new();
    for n in 0..count {
        ids.push(tasks.create(timed(n)).await.unwrap().id);
    }
    ids
}

/// Every page of `query`, as the titles.
async fn all_pages(pages: &dyn TaskPages, query: &TaskQuery) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut page = pages.first(query).await.unwrap();
    loop {
        let titles = page.items.iter().map(|r| r.entry.task.display_title());
        out.push(titles.collect());
        match page.next {
            Some(cursor) => page = pages.next(query, &cursor).await.unwrap(),
            None => return out,
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn more_than_one_page_in_order_and_the_last_has_no_next() {
    let served = Served::start().await;
    let tasks = served.client();
    fill(&tasks, 230).await;
    let pages = all_pages(&tasks, &TaskQuery::new()).await;
    let sizes: Vec<usize> = pages.iter().map(Vec::len).collect();
    assert_eq!(sizes, [100, 100, 30]);
    let expected: Vec<String> = (0..230).map(|n| format!("T {n:04}")).collect();
    assert_eq!(pages.concat(), expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn exactly_one_full_page_ends_without_an_empty_page_after_it() {
    let served = Served::start().await;
    let tasks = served.client();
    fill(&tasks, 100).await;
    let first = tasks.first(&TaskQuery::new()).await.unwrap();
    assert_eq!(first.items.len(), 100);
    assert!(first.next.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_list_is_one_empty_page_without_next() {
    let served = Served::start().await;
    let tasks = served.client();
    let first = tasks.first(&TaskQuery::new()).await.unwrap();
    assert!(first.items.is_empty() && first.next.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_query_pages_as_well_and_only_what_matches() {
    let served = Served::start().await;
    let tasks = served.client();
    for n in 0..130 {
        let mut t = timed(n);
        t.progress = Some(if n < 120 { "completed" } else { "needs-action" }.into());
        tasks.create(t).await.unwrap();
    }
    let done = all_pages(&tasks, &TaskQuery::new().with_progress("completed")).await;
    assert_eq!(done.iter().map(Vec::len).collect::<Vec<_>>(), [100, 20]);
    let open = all_pages(&tasks, &TaskQuery::new().with_progress("needs-action")).await;
    assert_eq!(open.iter().map(Vec::len).collect::<Vec<_>>(), [10]);
    assert!(all_pages(&tasks, &TaskQuery::new().with_label("none")).await[0].is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_page_carries_the_revisions_of_the_server() {
    let served = Served::start().await;
    let tasks = served.client();
    fill(&tasks, 3).await;
    let page = tasks.first(&TaskQuery::new()).await.unwrap();
    for read in &page.items {
        let again = tasks.get_revised(read.entry.id).await.unwrap().unwrap();
        assert_eq!(read.revision, again.revision);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cursor_that_the_server_did_not_hand_out_is_a_clear_error() {
    let served = Served::start().await;
    let tasks = served.client();
    fill(&tasks, 3).await;
    for bytes in [vec![9, 9], vec![1, 2, 3], vec![7; 9], b"nonsense".to_vec()] {
        let got = tasks
            .next(&TaskQuery::new(), &Cursor::from_bytes(bytes))
            .await;
        match got {
            Err(Error::Store(fs_store::Error::Invalid(text))) => {
                assert!(text.contains("cursor"), "{text}");
            }
            other => panic!("expected an invalid cursor, got {other:?}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_task_added_before_the_boundary_comes_round_once_more() {
    let served = Served::start().await;
    let tasks = served.client();
    fill(&tasks, 150).await;
    let query = TaskQuery::new();
    let first = tasks.first(&query).await.unwrap();
    // The cursor is a place in the list today (`fs-interfaces` page.rs): a task that sorts before
    // everything moves the boundary by one, so the last of page one comes again. Nothing errs.
    let mut early = task("uid-early", "Early");
    early.due = Some("2026-11-30T10:00:00".into());
    tasks.create(early).await.unwrap();
    let second = tasks.next(&query, &first.next.unwrap()).await.unwrap();
    assert_eq!(second.items.len(), 51);
    assert_eq!(second.items[0].entry.id, first.items[99].entry.id);
    assert!(second.next.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_task_removed_before_the_boundary_skips_the_next_one() {
    let served = Served::start().await;
    let tasks = served.client();
    let ids = fill(&tasks, 150).await;
    let query = TaskQuery::new();
    let first = tasks.first(&query).await.unwrap();
    tasks.delete(ids[0]).await.unwrap();
    let second = tasks.next(&query, &first.next.unwrap()).await.unwrap();
    // 149 are left; the place 100 is now the 101st of the old list: T 0100 was skipped.
    assert_eq!(second.items.len(), 49);
    assert_eq!(second.items[0].entry.task.display_title(), "T 0101");
    assert!(second.next.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_walk_gives_page_after_page_then_none_and_keeps_saying_none() {
    let served = Served::start().await;
    let tasks = Arc::new(served.client());
    fill(tasks.as_ref(), 130).await;
    let mut walk = PageWalk::new(tasks, TaskQuery::new());
    assert_eq!(walk.next_page().await.unwrap().unwrap().items.len(), 100);
    assert_eq!(walk.next_page().await.unwrap().unwrap().items.len(), 30);
    assert!(walk.next_page().await.unwrap().is_none());
    assert!(walk.next_page().await.unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_walk_that_met_an_error_stays_where_it_was() {
    let mut served = Served::start().await;
    let tasks = Arc::new(served.client());
    fill(tasks.as_ref(), 130).await;
    let mut walk = PageWalk::new(tasks.clone(), TaskQuery::new());
    assert_eq!(walk.next_page().await.unwrap().unwrap().items.len(), 100);
    served.stop();
    assert!(walk.next_page().await.is_err());
    assert!(walk.next_page().await.is_err(), "still at the same place");
}
