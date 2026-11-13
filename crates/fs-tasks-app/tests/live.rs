//! The board follows the server: a change by another client shows up without anybody asking.
//!
//! The way is the server's own change feed (`Tasks.subscribe`, a `ChangeListener`), not polling.

use std::sync::Arc;
use std::time::{Duration, Instant};

use fs_objects::{Store, TursoStore};
use fs_tasks_app::Day;
use fs_tasks_app::plan_view::Cache;
use fs_tasks_core::{Task, TaskItems, TaskRepo};
use fs_tasks_server::{RemoteItems, host};
use serde_json::json;

fn task(title: &str) -> Task {
    Task::from_value(json!({
        "@type": "Task", "version": "2.0", "uid": format!("urn:uuid:{title}"), "title": title,
    }))
    .unwrap()
}

fn in_progress(cache: &Cache) -> bool {
    cache.current().is_some_and(|s| {
        s.entries
            .iter()
            .any(|e| e.task.effective_progress() == "in-process")
    })
}

fn wait_for(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    done()
}

#[test]
fn a_card_moves_to_in_progress_when_another_client_changes_the_task() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let repo = rt.block_on(async {
        let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
        Arc::new(TaskRepo::new(store, "https://localhost/actors/owner"))
    });
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("run").join("tasks.sock");
    let _hosted = host(repo.clone(), socket.clone()).unwrap();
    let today = Day::parse("2026-11-12").unwrap();

    // Two windows over the same server: one wired to the change feed, one not (the counter test).
    let make = |follow: bool| {
        let remote = RemoteItems::connect(socket.clone(), None).unwrap();
        let changes = remote.changes();
        let cache = Cache::start(Arc::new(remote), rt.handle().clone(), today);
        if follow {
            let again = Arc::clone(&cache);
            changes.on_change(move || again.reload_in_background());
        }
        cache.wait(Duration::from_secs(5)).expect("first picture");
        cache
    };
    let live = make(true);
    let deaf = make(false);
    assert!(!in_progress(&live));

    // Another client (the command line, an agent) creates a task and starts work on it.
    let entry = rt.block_on(repo.create(task("one"))).unwrap();
    let mut started = entry.task.clone();
    started.progress = Some("in-process".into());
    rt.block_on(repo.edit(entry.id, started)).unwrap();

    assert!(
        wait_for(Duration::from_secs(5), || in_progress(&live)),
        "the card did not move within 5 s"
    );
    // Without the feed nothing renews the picture: the test would pass for any cache otherwise.
    assert!(!in_progress(&deaf));
}
