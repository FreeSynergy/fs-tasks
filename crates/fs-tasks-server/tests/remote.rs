//! The window's side of the socket: `RemoteItems` against a served repository.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use fs_objects::{Store, TursoStore};
use fs_tasks_core::{Task, TaskItems, TaskQuery, TaskRepo};
use fs_tasks_server::{RemoteItems, host};
use serde_json::json;

fn task(title: &str) -> Task {
    Task::from_value(json!({
        "@type": "Task", "version": "2.0", "uid": format!("urn:uuid:{title}"), "title": title,
    }))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn reads_writes_and_hears_changes_over_the_socket() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let repo = Arc::new(TaskRepo::new(store, "https://local.invalid/users/owner"));
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("run").join("tasks.sock");
    let _hosted = host(repo.clone(), socket.clone()).unwrap();

    let remote = RemoteItems::connect(socket, None).unwrap();
    assert!(remote.capabilities().is_supported("task.create"));
    let heard = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&heard);
    remote.changes().on_change(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });

    assert_eq!(remote.list().await.unwrap().len(), 0);
    let made = remote.create(task("a")).await.unwrap();
    assert_eq!(remote.get(made.id).await.unwrap().unwrap().id, made.id);
    assert_eq!(remote.list().await.unwrap().len(), 1);
    assert_eq!(
        remote
            .query(&TaskQuery::new().matching("a"))
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        remote
            .query(&TaskQuery::new().matching("zzz"))
            .await
            .unwrap()
            .len(),
        0
    );
    // The write went into the repository the server holds: one store, not a copy.
    assert_eq!(repo.list().await.unwrap().len(), 1);

    // A change by somebody else is heard (and so is the own one above: at least two notices).
    repo.create(task("b")).await.unwrap();
    let start = Instant::now();
    while heard.load(Ordering::SeqCst) < 2 && start.elapsed() < Duration::from_secs(5) {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(heard.load(Ordering::SeqCst) >= 2);

    remote.delete(made.id).await.unwrap();
    assert!(remote.get(made.id).await.unwrap().is_none());
    // What the wire does not carry is said, not faked.
    assert!(remote.page(&TaskQuery::new(), None, 10).await.is_err());
}
