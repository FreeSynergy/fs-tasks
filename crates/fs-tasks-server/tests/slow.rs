//! A call that gets no answer in time ends with a message: a hanging server must not freeze the window.

use std::sync::Arc;
use std::time::Duration;

use fs_objects::{Store, TursoStore};
use fs_tasks_core::{TaskItems, TaskRepo};
use fs_tasks_server::{RemoteItems, host};

#[tokio::test(flavor = "multi_thread")]
async fn a_call_without_an_answer_in_time_says_so() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let repo = Arc::new(TaskRepo::new(store, "https://local.invalid/users/owner"));
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("run").join("tasks.sock");
    let _hosted = host(repo, socket.clone()).unwrap();

    // The answer of a real server needs a round trip, which cannot be done in a nanosecond: it stands for a server
    // that does not answer.
    let remote = RemoteItems::connect(socket, None)
        .unwrap()
        .with_call_limit(Duration::from_nanos(1));
    let said = remote.list().await.unwrap_err().to_string();
    assert!(said.contains("did not answer within"), "{said}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_counter_test_with_the_normal_limit_the_same_call_answers() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let repo = Arc::new(TaskRepo::new(store, "https://local.invalid/users/owner"));
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("run").join("tasks.sock");
    let _hosted = host(repo, socket.clone()).unwrap();
    let remote = RemoteItems::connect(socket, None).unwrap();
    assert!(remote.list().await.unwrap().is_empty());
}
