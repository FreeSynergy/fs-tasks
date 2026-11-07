//! The real binary against the real server: it connects by `--socket`, and without one it says how.
//!
//! The tests start the process; no window is opened (export only).

mod common;

use std::process::Command;

use common::{Scratch, World};
use fs_tasks_core::{Task, TaskItems};

fn app() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fs-tasks-app"))
}

#[test]
fn without_a_socket_it_stops_and_names_the_server() {
    let out = app().output().unwrap();
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("fs-tasks-server --socket"), "{said}");
}

#[test]
fn the_store_option_is_refused_and_points_to_the_data_of_the_server() {
    let out = app().args(["--store", "x.db"]).output().unwrap();
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("--data"), "{said}");
}

#[test]
fn a_socket_nobody_listens_on_is_an_error_in_words() {
    let dir = Scratch::new();
    let out = app()
        .arg("--socket")
        .arg(dir.file("none.sock"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("cannot reach the tasks server"), "{said}");
}

#[test]
fn it_imports_and_exports_through_the_server() {
    let w = World::new();
    w.add(Task::new("Alpha"));
    let dir = Scratch::new();
    let (out_file, ics) = (dir.file("out.ics"), dir.file("in.ics"));
    std::fs::write(
        &ics,
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:Beta\r\nEND:VTODO\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    let out = app()
        .arg("--socket")
        .arg(&w.socket)
        .arg("--import")
        .arg(&ics)
        .arg("--export")
        .arg(&out_file)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = std::fs::read_to_string(&out_file).unwrap();
    assert!(
        text.contains("SUMMARY:Alpha") && text.contains("SUMMARY:Beta"),
        "{text}"
    );
    assert_eq!(
        w.runtime.block_on(w.tasks.list()).unwrap().len(),
        2,
        "the server holds both"
    );
}
