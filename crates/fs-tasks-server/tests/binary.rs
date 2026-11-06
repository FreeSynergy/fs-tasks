//! The real program: started, used, stopped and started again; the tasks are still there.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use fs_interfaces::rpc::connect;
use fs_interfaces::tasks_capnp::tasks;

fn start(socket: &Path, data: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_fs-tasks-server"))
        .arg("--socket")
        .arg(socket)
        .arg("--data")
        .arg(data)
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

async fn wait_for(socket: &Path) {
    for _ in 0..200 {
        if tokio::net::UnixStream::connect(socket).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the server did not come up");
}

fn terminate(mut child: Child) {
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(child.id().to_string())
        .status()
        .unwrap();
    assert!(status.success());
    assert!(child.wait().unwrap().success());
}

async fn stored_tasks(socket: &Path) -> usize {
    let connection = connect::<tasks::Client>(socket).await.unwrap();
    let answer = connection.items_request().send().promise.await.unwrap();
    let items = answer.get().unwrap().get_items().unwrap();
    let listed = items.list_request().send().promise.await.unwrap();
    listed
        .get()
        .unwrap()
        .get_page()
        .unwrap()
        .get_items()
        .unwrap()
        .len() as usize
}

#[tokio::test(flavor = "current_thread")]
async fn tasks_survive_a_restart_and_a_second_start_is_refused() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let socket = dir.path().join("tasks.sock");
            let data = dir.path().join("data");

            let first = start(&socket, &data);
            wait_for(&socket).await;

            let mut second = start(&socket, &data);
            assert!(!second.wait().unwrap().success(), "a second server started");
            assert!(socket.exists(), "the second start touched the first");

            let connection = connect::<tasks::Client>(&socket).await.unwrap();
            let answer = connection.items_request().send().promise.await.unwrap();
            let items = answer.get().unwrap().get_items().unwrap();
            let mut create = items.create_request();
            create
                .get()
                .set_jscalendar(r#"{"@type":"Task","version":"2.0","uid":"kept","title":"Kept"}"#);
            create.send().promise.await.unwrap();
            drop(answer);
            drop(connection);

            terminate(first);
            assert!(!socket.exists(), "the socket stayed behind");

            let again = start(&socket, &data);
            wait_for(&socket).await;
            assert_eq!(stored_tasks(&socket).await, 1);
            terminate(again);
        })
        .await;
}
