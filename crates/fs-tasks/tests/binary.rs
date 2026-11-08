//! The real program: started, used, stopped and started again; the tasks are still there.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use fs_interfaces::rpc::{connect, read_envelope};
use fs_interfaces::tasks_capnp::tasks;

fn start(socket: &Path, data: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_fs-tasks"))
        .arg("serve")
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
    let answer = connection
        .client
        .items_request()
        .send()
        .promise
        .await
        .unwrap();
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
            let mode = std::fs::metadata(&data).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "the data is for this user alone");

            let mut second = start(&socket, &data);
            assert_eq!(
                second.wait().unwrap().code(),
                Some(1),
                "a second server started"
            );
            assert!(socket.exists(), "the second start touched the first");

            let connection = connect::<tasks::Client>(&socket).await.unwrap();
            let answer = connection
                .client
                .items_request()
                .send()
                .promise
                .await
                .unwrap();
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

#[tokio::test(flavor = "current_thread")]
async fn the_instance_names_the_iri_of_a_new_task() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let socket = dir.path().join("tasks.sock");
            let server = Command::new(env!("CARGO_BIN_EXE_fs-tasks"))
                .arg("serve")
                .arg("--socket")
                .arg(&socket)
                .arg("--data")
                .arg(dir.path().join("data"))
                .args(["--instance", "tasks.example.org"])
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            wait_for(&socket).await;
            let connection = connect::<tasks::Client>(&socket).await.unwrap();
            let answer = connection
                .client
                .items_request()
                .send()
                .promise
                .await
                .unwrap();
            let items = answer.get().unwrap().get_items().unwrap();
            let mut create = items.create_request();
            create
                .get()
                .set_jscalendar(r#"{"@type":"Task","version":"2.0","uid":"i","title":"I"}"#);
            let created = create.send().promise.await.unwrap();
            let task = created.get().unwrap().get_task().unwrap();
            let envelope = read_envelope(task.get_envelope().unwrap()).unwrap();
            let iri = envelope.iri.unwrap();
            drop(created);
            drop(answer);
            drop(connection);
            // Stopped first: a failed assertion must not leave the server running.
            terminate(server);
            assert!(iri.starts_with("https://tasks.example.org/tasks/"), "{iri}");
        })
        .await;
}

#[test]
fn it_refuses_an_instance_that_is_no_domain() {
    let dir = tempfile::tempdir().unwrap();
    for bad in [
        "a/b",
        "x y",
        "user@example.org",
        "example.org:80",
        "-a.example",
    ] {
        let code = Command::new(env!("CARGO_BIN_EXE_fs-tasks"))
            .arg("serve")
            .arg("--socket")
            .arg(dir.path().join("x.sock"))
            .arg("--data")
            .arg(dir.path().join("data"))
            .args(["--instance", bad])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .code();
        assert_eq!(code, Some(2), "{bad}");
    }
    assert!(!dir.path().join("data").exists(), "nothing was opened");
}

#[test]
fn it_refuses_bad_arguments_and_a_data_directory_others_can_read() {
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_fs-tasks"))
            .arg("serve")
            .args(args)
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .unwrap()
            .code()
    };
    assert_eq!(run(&["--help"]), Some(0));
    assert_eq!(run(&["--nonsense"]), Some(2));
    assert_eq!(run(&["--socket"]), Some(2));
    assert_eq!(
        run(&["--socket", "/s", "--data", "/d", "--actor", ""]),
        Some(2)
    );

    let dir = tempfile::tempdir().unwrap();
    let open = dir.path().join("open");
    std::fs::create_dir(&open).unwrap();
    std::fs::set_permissions(&open, PermissionsExt::from_mode(0o755)).unwrap();
    let socket = dir.path().join("x.sock");
    let (socket, open) = (socket.to_str().unwrap(), open.to_str().unwrap());
    assert_eq!(run(&["--socket", socket, "--data", open]), Some(1));
}
