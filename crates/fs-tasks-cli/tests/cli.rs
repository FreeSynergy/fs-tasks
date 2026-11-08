//! The commands against a real server on a real socket (in this process).

use std::sync::Arc;

use fs_interfaces::rpc::{bind_socket, connect, serve};
use fs_interfaces::tasks_capnp::tasks;
use fs_objects::{Store, TursoStore};
use fs_tasks_cli::{Command, run};
use fs_tasks_core::TaskRepo;
use fs_tasks_server::TasksServer;

async fn with_cli<F, Fut>(test: F)
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

fn add(title: &str) -> Command {
    Command::Add {
        title: title.into(),
        due: Some("2026-12-01T10:00:00".into()),
        zone: Some("Europe/Berlin".into()),
        label: Some("work".into()),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn add_list_show_done_delete() {
    with_cli(|root| async move {
        let added = run(&root, add("Plan")).await.unwrap();
        let id = added.split("  ").next().unwrap().to_string();
        assert!(added.ends_with("  Plan"), "{added}");

        let listed = run(&root, Command::List).await.unwrap();
        assert_eq!(listed, added, "one task, the same line");

        let shown = run(&root, Command::Show(id.clone())).await.unwrap();
        assert!(shown.contains("\"title\": \"Plan\""), "{shown}");
        assert!(shown.contains("\"work\""), "{shown}");

        let finished = run(&root, Command::Done(id.clone())).await.unwrap();
        assert!(finished.contains("  completed  "), "{finished}");

        let gone = run(&root, Command::Delete(id.clone())).await.unwrap();
        assert_eq!(gone, format!("deleted {id}"));
        assert_eq!(run(&root, Command::List).await.unwrap(), "");
        let missing = run(&root, Command::Show(id)).await;
        assert!(missing.is_err());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn export_import_and_capabilities() {
    with_cli(|root| async move {
        run(&root, add("One")).await.unwrap();
        let ics = run(&root, Command::Export).await.unwrap();
        assert!(ics.contains("BEGIN:VTODO") && ics.contains("SUMMARY:One"), "{ics}");

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("in.ics");
        std::fs::write(
            &file,
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:Minutes\r\nEND:VTODO\r\nEND:VCALENDAR\r\n",
        )
        .unwrap();
        let report = run(&root, Command::Import(file.clone())).await.unwrap();
        assert_eq!(report, "created 1, skipped 0, failed 0");
        let again = run(&root, Command::Import(file)).await.unwrap();
        assert_eq!(again, "created 0, skipped 1, failed 0");

        let caps = run(&root, Command::Capabilities).await.unwrap();
        assert!(caps.lines().any(|l| l == "task.create"), "{caps}");
        assert!(run(&root, Command::Show("not-an-id".into())).await.is_err());
    })
    .await;
}
