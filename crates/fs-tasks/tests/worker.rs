//! The real program as a worker tool: no server started by hand, many clients at once, one store.

use std::path::Path;
use std::process::{Command, Output};

fn fs_tasks(dir: &Path, args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_fs-tasks"));
    // The defaults of a user: socket in the runtime folder, data in the data folder.
    c.env("XDG_RUNTIME_DIR", dir.join("run"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env_remove("FS_TASKS_SOCKET")
        .args(args);
    c
}

fn run(dir: &Path, args: &[&str]) -> Output {
    fs_tasks(dir, args).output().unwrap()
}

/// The processes `fs-tasks serve` that use this test's folder.
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

/// Stops the autostarted server whatever happens in the test.
struct Cleanup<'a>(&'a Path);

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        for pid in servers(self.0) {
            let _ = Command::new("kill").args(["-TERM", &pid.to_string()]).status();
        }
    }
}

#[test]
fn clients_start_one_server_by_themselves_and_share_one_store() {
    let dir = tempfile::tempdir().unwrap();
    let _stop = Cleanup(dir.path());
    // Four clients at once and no server: every one of them works, and one server serves them.
    let path = dir.path();
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let path = path.to_path_buf();
            std::thread::spawn(move || {
                run(&path, &["add", &format!("Job {i}"), "--project", "p", "--json"])
            })
        })
        .collect();
    let outs: Vec<Output> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    for o in &outs {
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }
    assert_eq!(servers(dir.path()).len(), 1, "one server, not one per client");

    let listed = run(dir.path(), &["list", "--project", "p", "--json"]);
    let listed = String::from_utf8(listed.stdout).unwrap();
    assert_eq!(listed.matches("\"id\":").count(), 4, "{listed}");
    let id = listed.split("\"id\":\"").nth(1).unwrap().split('"').next().unwrap().to_string();

    // Four workers claim the same task at once: one gets exit 0, the others exit 3 naming the holder.
    let claims: Vec<_> = (0..4)
        .map(|i| {
            let (path, id) = (path.to_path_buf(), id.clone());
            std::thread::spawn(move || run(&path, &["claim", &id, "--as", &format!("w{i}"), "--json"]))
        })
        .collect();
    let codes: Vec<(Option<i32>, String)> = claims
        .into_iter()
        .map(|h| h.join().unwrap())
        .map(|o| (o.status.code(), String::from_utf8_lossy(&o.stderr).into_owned()))
        .collect();
    assert_eq!(codes.iter().filter(|c| c.0 == Some(0)).count(), 1, "{codes:?}");
    let lost: Vec<_> = codes.iter().filter(|c| c.0 == Some(3)).collect();
    assert_eq!(lost.len(), 3, "{codes:?}");
    assert!(lost.iter().all(|c| c.1.contains("\"conflict\":true") && c.1.contains("claimed by")));

    // The data folder is the one of the window: the file the server opened is `objects.db` there.
    assert!(dir.path().join("data/freesynergy/tasks/objects.db").exists());
    // One process holds the store: a window started now (next to the autostarted server) says so instead of
    // opening a second copy. (A window started FIRST answers the command line itself; see fs-tasks-server `host`.)
    let store = dir.path().join("data/freesynergy/tasks/objects.db");
    let ics = dir.path().join("window.ics");
    let seen = run(
        dir.path(),
        &["app", "--store", store.to_str().unwrap(), "--export", ics.to_str().unwrap()],
    );
    assert!(!seen.status.success());
    assert!(String::from_utf8_lossy(&seen.stderr).contains("another fs-tasks holds it"));
    // No ANSI escapes in anything a script reads.
    for o in &outs {
        assert!(!o.stdout.contains(&0x1b) && !o.stderr.contains(&0x1b));
    }
}
