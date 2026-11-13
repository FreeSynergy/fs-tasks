//! A check that runs too long is ended with everything it started (Unix: the process group).

#![cfg(unix)]

use std::path::Path;
use std::time::{Duration, Instant};

use fs_tasks_cli::execute;

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(ToString::to_string).collect()
}

fn alive(pid: &str) -> bool {
    // A zombie is not alive: its state is `Z`.
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .is_ok_and(|s| s.rsplit(") ").next().is_some_and(|rest| !rest.starts_with('Z')))
}

fn pid_of(file: &Path) -> String {
    let start = Instant::now();
    loop {
        if let Ok(text) = std::fs::read_to_string(file)
            && !text.trim().is_empty()
        {
            return text.trim().to_string();
        }
        assert!(start.elapsed() < Duration::from_secs(5), "no pid written");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn gone(pid: &str) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn after_the_time_limit_neither_the_child_nor_its_grandchild_lives() {
    let dir = tempfile::tempdir().unwrap();
    let (script, pids) = (dir.path().join("grand.sh"), dir.path().join("pids"));
    // The script starts a grandchild in the background and waits for it.
    std::fs::write(
        &script,
        "echo $$ > \"$1\"; sleep 300 & echo $! >> \"$1\"; wait\n",
    )
    .unwrap();
    let out = execute(
        &argv(&["bash", script.to_str().unwrap(), pids.to_str().unwrap()]),
        1,
    )
    .unwrap();
    assert!(out.timed_out && !out.passed);
    let text = std::fs::read_to_string(&pids).unwrap();
    let ids: Vec<&str> = text.split_whitespace().collect();
    assert_eq!(ids.len(), 2, "{text:?}");
    for pid in ids {
        assert!(gone(pid), "process {pid} still lives");
    }
}

#[test]
fn the_counter_test_a_grandchild_left_behind_by_a_finished_check_is_seen_alive() {
    // Without the time limit nothing ends the grandchild: the test above would be blind if `alive` always said no.
    let dir = tempfile::tempdir().unwrap();
    let (script, pidfile) = (dir.path().join("left.sh"), dir.path().join("pid"));
    std::fs::write(
        &script,
        "sleep 300 >/dev/null 2>&1 & echo $! > \"$1\"\n",
    )
    .unwrap();
    let out = execute(
        &argv(&["bash", script.to_str().unwrap(), pidfile.to_str().unwrap()]),
        10,
    )
    .unwrap();
    assert!(out.passed);
    let pid = pid_of(&pidfile);
    assert!(alive(&pid));
    let _ = std::process::Command::new("kill").arg(&pid).status();
    assert!(gone(&pid));
}
