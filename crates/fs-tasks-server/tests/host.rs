//! The window's way of serving: the same repository, on a socket, from a thread of its own.

use std::sync::Arc;

use fs_interfaces::rpc::connect;
use fs_interfaces::tasks_capnp::tasks;
use fs_objects::{Store, TursoStore};
use fs_tasks_core::{TaskItems, TaskRepo};
use fs_tasks_server::host;

#[tokio::test(flavor = "current_thread")]
async fn a_client_on_the_socket_changes_the_repository_the_host_holds() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let repo = Arc::new(TaskRepo::new(store, "https://local.invalid/users/owner"));
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("run").join("tasks.sock");

    let hosted = host(repo.clone(), socket.clone()).unwrap();
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let connection = connect::<tasks::Client>(&socket).await.unwrap();
            let answer = connection.client.items_request().send().promise.await.unwrap();
            let items = answer.get().unwrap().get_items().unwrap();
            let mut create = items.create_request();
            create
                .get()
                .set_jscalendar(r#"{"@type":"Task","version":"2.0","uid":"w","title":"From a worker"}"#);
            create.send().promise.await.unwrap();
            drop(answer);
            drop(connection);
        })
        .await;
    // The window's own handle sees it at once: one repository, not a copy.
    let tasks = repo.list().await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task.title.as_deref(), Some("From a worker"));

    // A second host on the same socket is refused and touches nothing.
    let again = host(repo.clone(), socket.clone());
    assert!(again.is_err());
    assert!(socket.exists());
    drop(hosted);
    assert!(!socket.exists(), "the socket is removed when the host ends");
}
