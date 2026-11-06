//! Shared helpers of the integration tests: a real server on a real Unix socket.

#![allow(dead_code)] // each test file uses a different subset

use std::sync::Arc;

use fs_interfaces::common_capnp;
use fs_interfaces::rpc::{bind_socket, connect, read_envelope, serve, write_uuid};
use fs_interfaces::tasks_capnp::{self, items, tasks};
use fs_store::{Store, TursoStore};
use fs_tasks_core::{TaskFeed, TaskItems, TaskRepo};
use fs_tasks_server::TasksServer;
use uuid::Uuid;

pub const ME: &str = "https://local.invalid/users/owner";
pub const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261112T170000Z\r\nSUMMARY:Minutes\r\nSTATUS:NEEDS-ACTION\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";

pub fn json(uid: &str, title: &str) -> String {
    format!(
        r#"{{"@type":"Task","version":"2.0","uid":"{uid}","title":"{title}","due":"2026-12-01T10:00:00","timeZone":"Europe/Berlin"}}"#
    )
}

pub async fn with_server<F, Fut>(test: F)
where
    F: FnOnce(tasks::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let repo = Arc::new(TaskRepo::new(store, ME));
    with_provider(repo.clone(), repo, test).await;
}

/// A real server in front of this provider.
pub async fn with_provider<F, Fut>(items: Arc<dyn TaskItems>, feed: Arc<dyn TaskFeed>, test: F)
where
    F: FnOnce(tasks::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("tasks.sock");
            let root = TasksServer::new(items, feed).into_client();
            let bound = bind_socket(&path).await.unwrap();
            let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
            let serving = tokio::task::spawn_local(serve(bound, root.client, async {
                let _ = stopped.await;
            }));
            let connection = connect::<tasks::Client>(&path).await.unwrap();
            test(connection.client.clone()).await;
            drop(connection);
            let _ = stop.send(());
            serving.await.unwrap().unwrap();
        })
        .await;
}

pub async fn items_of(root: &tasks::Client) -> items::Client {
    root.items_request()
        .send()
        .promise
        .await
        .unwrap()
        .get()
        .unwrap()
        .get_items()
        .unwrap()
}

/// Create a task and answer its object id.
pub async fn create(items: &items::Client, uid: &str, title: &str) -> Uuid {
    let mut req = items.create_request();
    req.get().set_jscalendar(json(uid, title).as_str());
    let answer = req.send().promise.await.unwrap();
    let task = answer.get().unwrap().get_task().unwrap();
    let env = read_envelope(task.get_envelope().unwrap()).unwrap();
    assert_eq!(env.name, title);
    assert_eq!(env.context.as_deref(), Some("tasks"));
    env.id.as_uuid()
}

/// Write a task id.
pub fn set_id(out: common_capnp::uuid::Builder<'_>, id: Uuid) {
    write_uuid(out, Some(id));
}

/// The JSCalendar content of a task on the wire.
pub fn task_json(task: tasks_capnp::task::Reader<'_>) -> serde_json::Value {
    serde_json::from_str(task.get_jscalendar().unwrap().to_str().unwrap()).unwrap()
}
