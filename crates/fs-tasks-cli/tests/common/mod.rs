//! A real server on a real socket (in this process), and a few helpers for the tests of the commands.
#![allow(dead_code)]

use std::sync::Arc;

use fs_interfaces::rpc::{bind_socket, connect, serve};
use fs_interfaces::tasks_capnp::{items, tasks};
use fs_objects::{Store, TursoStore};
use fs_tasks_cli::{Command, Failure, Policy, parse, run_with};
use fs_tasks_core::TaskRepo;
use fs_tasks_server::TasksServer;
use serde_json::Value;

pub async fn with_cli<F, Fut>(test: F)
where
    F: FnOnce(tasks::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
            let repo = Arc::new(TaskRepo::new(store, "https://local.invalid/users/owner"));
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("tasks.sock");
            let root = TasksServer::new(repo.clone(), repo).into_client();
            let bound = bind_socket(&path).await.unwrap();
            let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
            let serving = tokio::task::spawn_local(serve(bound, root.client, async {
                let _ = stopped.await;
            }));
            let connection = connect::<tasks::Client>(&path).await.unwrap();
            test(connection.client.clone()).await;
            drop(connection);
            let _ = stop.send(());
            let _ = serving.await;
        })
        .await;
}

/// The command line `words` against `root`, with the default settings plus `programs`. The words are parsed at
/// once, so the future holds nothing but `root`.
pub fn cmd<'a>(
    root: &'a tasks::Client,
    words: &str,
    programs: &[&str],
) -> impl Future<Output = Result<String, Failure>> + 'a {
    let words: Vec<String> = words.split_whitespace().map(String::from).collect();
    let command: Command = parse(&words).unwrap().command.unwrap();
    let policy = Policy {
        programs: programs.iter().map(|p| (*p).to_string()).collect(),
        ..Policy::default()
    };
    async move { run_with(root, command, &Ok(policy)).await }
}

/// The first word of a plain output line, or the `id` of a JSON output.
pub fn id_of(out: &str) -> String {
    match serde_json::from_str::<Value>(out) {
        Ok(v) => v["id"].as_str().unwrap().to_string(),
        Err(_) => out.split("  ").next().unwrap().to_string(),
    }
}

/// A task as the wire takes it, made without `add` (so with whatever members the test gives it).
pub async fn raw_create(root: &tasks::Client, task: &Value) -> String {
    let items: items::Client = root
        .items_request()
        .send()
        .promise
        .await
        .unwrap()
        .get()
        .unwrap()
        .get_items()
        .unwrap();
    let mut req = items.create_request();
    req.get().set_jscalendar(task.to_string().as_str());
    let answer = req.send().promise.await.unwrap();
    let id = fs_interfaces::rpc::read_envelope(
        answer
            .get()
            .unwrap()
            .get_task()
            .unwrap()
            .get_envelope()
            .unwrap(),
    )
    .unwrap()
    .id;
    id.to_string()
}
