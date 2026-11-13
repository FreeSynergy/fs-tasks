//! The window is a client of the one server of the device (the real program, a real picture of the window).

#![cfg(feature = "gui")]

use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

fn fs_tasks(dir: &Path, args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_fs-tasks"));
    c.env("XDG_RUNTIME_DIR", dir.join("run"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env_remove("FS_TASKS_SOCKET")
        .args(args);
    c
}

fn run(dir: &Path, args: &[&str]) -> Output {
    fs_tasks(dir, args).output().unwrap()
}

fn socket(dir: &Path) -> String {
    dir.join("run/freesynergy/tasks.sock").display().to_string()
}

/// The `fs-tasks serve` processes that use this test's folder.
fn servers(dir: &Path) -> Vec<u32> {
    let needle = dir.join("run").display().to_string();
    let mut found = vec![];
    for entry in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(raw) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let text = String::from_utf8_lossy(&raw).replace('\0', " ");
        if text.contains(" serve ") && text.contains(&needle) {
            found.push(pid);
        }
    }
    found
}

struct Cleanup<'a>(&'a Path);

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        for pid in servers(self.0) {
            let _ = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
        }
    }
}

fn snapshot(dir: &Path, name: &str) -> Output {
    let png = dir.join(name);
    let out = run(
        dir,
        &[
            "--connect",
            &socket(dir),
            "--snapshot",
            png.to_str().unwrap(),
            "--size",
            "500x320",
            "--plan",
            "board",
        ],
    );
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(std::fs::metadata(&png).unwrap().len() > 100);
    out
}

#[test]
fn a_window_next_to_a_running_server_opens_without_a_second_lock() {
    let dir = tempfile::tempdir().unwrap();
    let _stop = Cleanup(dir.path());
    let mut server: Child = fs_tasks(
        dir.path(),
        &["serve", "--socket", &socket(dir.path()), "--data", dir.path().join("data/freesynergy/tasks").to_str().unwrap()],
    )
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .unwrap();
    let start = Instant::now();
    while !Path::new(&socket(dir.path())).exists() && start.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(run(dir.path(), &["add", "Before the window"]).status.success());

    // The window (a picture) and the command line both work with the server running; no second server starts.
    snapshot(dir.path(), "a.png");
    let listed = run(dir.path(), &["list"]);
    assert!(String::from_utf8_lossy(&listed.stdout).contains("Before the window"));
    assert_eq!(servers(dir.path()).len(), 1);
    assert!(server.try_wait().unwrap().is_none(), "the server is still there");
    let _ = server.kill();
    let _ = server.wait();
}

#[test]
fn a_window_without_a_server_starts_it_and_the_server_stays_for_the_others() {
    let dir = tempfile::tempdir().unwrap();
    let _stop = Cleanup(dir.path());
    snapshot(dir.path(), "b.png");
    // The window has ended; the server it started is the motor and stays, so the command line finds it.
    assert_eq!(servers(dir.path()).len(), 1, "one server, started by the window");
    let added = run(dir.path(), &["add", "After the window"]);
    assert!(added.status.success(), "{}", String::from_utf8_lossy(&added.stderr));
    assert_eq!(servers(dir.path()).len(), 1);
    // A second window sees the task the command line made, through the same server.
    snapshot(dir.path(), "c.png");
    assert_eq!(servers(dir.path()).len(), 1);
}
